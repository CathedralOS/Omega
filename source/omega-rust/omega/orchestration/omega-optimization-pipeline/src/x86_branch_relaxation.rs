use std::collections::BTreeMap;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_isa_x86_64::{
    X86_64SelectedFormEncodingError, encode_x86_64_terminal_selected_nonzero_branch_form,
    encode_x86_64_terminal_selected_short_nonzero_branch_form,
    validate_x86_64_terminal_selected_nonzero_branch_form,
    validate_x86_64_terminal_selected_short_nonzero_branch_form,
};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey,
    TerminalMachineEncodedControlEffect, TerminalMachineEncodedEffects,
    TerminalMachineEncodedMemoryEffect, TerminalMachineEncodedStackEffect,
    TerminalMachineEncodedTrapBehavior, TerminalSelectedBlockId, TerminalSelectedInstructionId,
};
use sha2::{Digest, Sha256};

use crate::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
    TerminalResolvedSelectedFormLayoutIdentity, TerminalResolvedSelectedFormRow,
    TerminalResolvedSelectedFunctionLayout, validate_optimized_resolved_selected_form_layout,
};

const RELAXATION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation.v2";
const REVISION_SCHEMA: &[u8] = b"omega.terminal.x86-branch-relaxation-revision.v2";

/// Explicit post-layout optimization policy. It is neither part of the
/// required baseline layout nor an encoder heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalX86BranchRelaxationPolicy {
    X86RelaxConditionalBranchesToRel8V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalX86BranchRelaxationIdentity([u8; 32]);

impl TerminalX86BranchRelaxationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalX86BranchRelaxationRevisionIdentity([u8; 32]);

impl TerminalX86BranchRelaxationRevisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalX86BranchRelaxationAttemptOutcome {
    AlreadyShort,
    NearDisplacementOutsideI8,
    SelectedForRelaxation,
}

/// One branch inspected in deterministic function/block/instruction order.
/// Attempts stop at the selected branch in a mutating iteration; the terminal
/// no-change iteration records the complete remaining scan.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalX86BranchRelaxationAttempt {
    pub iteration: u64,
    pub input: TerminalX86BranchRelaxationRevisionIdentity,
    pub instruction: TerminalSelectedInstructionId,
    pub offset: u64,
    pub byte_displacement: i64,
    pub encoded_bytes: u8,
    pub outcome: TerminalX86BranchRelaxationAttemptOutcome,
}

/// Exact evidence for one monotone six-byte-near to two-byte-short rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalX86BranchRelaxationAction {
    pub iteration: u64,
    pub input: TerminalX86BranchRelaxationRevisionIdentity,
    pub output: TerminalX86BranchRelaxationRevisionIdentity,
    pub instruction: TerminalSelectedInstructionId,
    pub old_offset: u64,
    pub new_offset: u64,
    pub old_displacement: i64,
    pub new_displacement: i64,
    pub old_bytes: Vec<u8>,
    pub new_bytes: Vec<u8>,
}

/// Immutable result of the explicit post-layout fixed point. The baseline
/// layout remains retained by identity; this carrier owns only the rewritten
/// function-relative roster and grants no baseline-layout, emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86BranchRelaxation {
    source: TerminalResolvedSelectedFormLayoutIdentity,
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pre_layout: crate::TerminalSelectedFormEncodingIdentity,
    target: NativeTarget,
    policy: TerminalX86BranchRelaxationPolicy,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    output: TerminalResolvedSelectedFormLayoutIdentity,
    output_revision: TerminalX86BranchRelaxationRevisionIdentity,
    identity: TerminalX86BranchRelaxationIdentity,
    attempts: Vec<TerminalX86BranchRelaxationAttempt>,
    actions: Vec<TerminalX86BranchRelaxationAction>,
    layout: StagedOptimizedResolvedSelectedFormLayout,
}

impl StagedOptimizedX86BranchRelaxation {
    pub const fn source(&self) -> TerminalResolvedSelectedFormLayoutIdentity {
        self.source
    }

    pub const fn selected(
        &self,
    ) -> omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> omega_machine_optimizer::TerminalPostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(&self) -> crate::TerminalSelectedFormEncodingIdentity {
        self.pre_layout
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn policy(&self) -> TerminalX86BranchRelaxationPolicy {
        self.policy
    }

    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }

    pub const fn usage(&self) -> OptimizationWorkUsage {
        self.usage
    }

    pub const fn output(&self) -> TerminalResolvedSelectedFormLayoutIdentity {
        self.output
    }

    pub const fn output_revision(&self) -> TerminalX86BranchRelaxationRevisionIdentity {
        self.output_revision
    }

    pub const fn identity(&self) -> TerminalX86BranchRelaxationIdentity {
        self.identity
    }

    pub fn attempts(&self) -> &[TerminalX86BranchRelaxationAttempt] {
        &self.attempts
    }

    pub fn actions(&self) -> &[TerminalX86BranchRelaxationAction] {
        &self.actions
    }

    pub fn functions(&self) -> &[TerminalResolvedSelectedFunctionLayout] {
        self.layout.functions()
    }

    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalX86BranchRelaxationWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedX86BranchRelaxationError {
    Source(OptimizedResolvedSelectedFormLayoutError),
    UnsupportedTarget(NativeTarget),
    BudgetExceeded(TerminalX86BranchRelaxationWorkAxis),
    DuplicateInstruction(TerminalSelectedInstructionId),
    MissingTargetBlock(TerminalSelectedBlockId),
    OffsetOverflow,
    NonContiguousBlock(TerminalSelectedBlockId),
    BranchFallthroughMismatch(TerminalSelectedInstructionId),
    MalformedBranch(TerminalSelectedInstructionId),
    BranchEffectsMismatch(TerminalSelectedInstructionId),
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

/// Stage the explicit x86 rel8 fixed point after independently replaying the
/// required baseline layout and all of its selected/post-allocation custody.
pub fn stage_optimized_x86_branch_relaxation<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, pre_layout, source,
    )
    .map_err(OptimizedX86BranchRelaxationError::Source)?;
    validate_roots(source, physical)?;
    let artifact = compute_relaxation(source, physical, budget)?;
    validate_optimized_x86_branch_relaxation(
        selected, machine, physical, pre_layout, source, &artifact,
    )?;
    Ok(artifact)
}

/// Independent replay does not call the production fixed-point driver. It
/// reconstructs the ordered scan, each shrink, every dense offset, the terminal
/// no-change sweep, work usage, revisions, and final receipt.
pub fn validate_optimized_x86_branch_relaxation<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    source: &StagedOptimizedResolvedSelectedFormLayout,
    artifact: &StagedOptimizedX86BranchRelaxation,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, pre_layout, source,
    )
    .map_err(OptimizedX86BranchRelaxationError::Source)?;
    validate_roots(source, physical)?;
    if artifact.source != source.identity()
        || artifact.selected != source.selected()
        || artifact.machine != source.machine()
        || artifact.pre_layout != source.pre_layout()
        || artifact.target != source.target()
        || artifact.policy != TerminalX86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1
        || !artifact.usage.within(artifact.budget)
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    let replayed = replay_relaxation(source, physical, artifact.budget)?;
    compare_replayed_evidence(artifact, &replayed)?;
    if artifact != &replayed {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

fn validate_roots(
    source: &StagedOptimizedResolvedSelectedFormLayout,
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

fn compare_replayed_evidence(
    artifact: &StagedOptimizedX86BranchRelaxation,
    replayed: &StagedOptimizedX86BranchRelaxation,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    compare_replayed_action_evidence(
        &artifact.attempts,
        &artifact.actions,
        &replayed.attempts,
        &replayed.actions,
    )?;
    if artifact.functions() != replayed.functions()
        || artifact.output != replayed.output
        || artifact.output_revision != replayed.output_revision
        || artifact.identity != replayed.identity
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

fn compare_replayed_action_evidence(
    attempts: &[TerminalX86BranchRelaxationAttempt],
    actions: &[TerminalX86BranchRelaxationAction],
    replayed_attempts: &[TerminalX86BranchRelaxationAttempt],
    replayed_actions: &[TerminalX86BranchRelaxationAction],
) -> Result<(), OptimizedX86BranchRelaxationError> {
    if attempts != replayed_attempts || actions != replayed_actions {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

fn compute_relaxation(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let mut functions = source.functions().to_vec();
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();
    let roots = RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    };
    loop {
        charge(
            &mut usage.iterations,
            budget.iterations(),
            TerminalX86BranchRelaxationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = revision_identity(roots, &functions);
        let previous_bytes = total_bytes(&functions)?;
        let mut selected_action = None;
        'scan: for (function_index, function) in functions.iter().enumerate() {
            for (block_index, block) in function.blocks.iter().enumerate() {
                for (instruction_index, row) in block.instructions.iter().enumerate() {
                    let Some(branch) = row.branch.as_deref() else {
                        continue;
                    };
                    charge(
                        &mut usage.rule_evaluations,
                        budget.rule_evaluations(),
                        TerminalX86BranchRelaxationWorkAxis::RuleEvaluations,
                    )?;
                    let (outcome, short_displacement) = inspect_production_branch(
                        &functions[function_index],
                        block_index,
                        instruction_index,
                        physical,
                    )?;
                    attempts.push(TerminalX86BranchRelaxationAttempt {
                        iteration,
                        input,
                        instruction: row.instruction,
                        offset: row.offset,
                        byte_displacement: branch.byte_displacement,
                        encoded_bytes: u8::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                        outcome,
                    });
                    if outcome == TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation {
                        charge(
                            &mut usage.candidates,
                            budget.candidates(),
                            TerminalX86BranchRelaxationWorkAxis::Candidates,
                        )?;
                        charge(
                            &mut usage.validation_steps,
                            budget.validation_steps(),
                            TerminalX86BranchRelaxationWorkAxis::ValidationSteps,
                        )?;
                        let short_displacement = short_displacement.ok_or(
                            OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction),
                        )?;
                        selected_action = Some((
                            function_index,
                            block_index,
                            instruction_index,
                            short_displacement,
                        ));
                        break 'scan;
                    }
                }
            }
        }
        let Some((function_index, block_index, instruction_index, displacement)) = selected_action
        else {
            break;
        };
        charge(
            &mut usage.commits,
            budget.commits(),
            TerminalX86BranchRelaxationWorkAxis::Commits,
        )?;
        let old =
            functions[function_index].blocks[block_index].instructions[instruction_index].clone();
        let encoded = encode_x86_64_terminal_selected_short_nonzero_branch_form(
            physical,
            old.alternative,
            displacement,
        )
        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
        functions[function_index].blocks[block_index].instructions[instruction_index].bytes =
            encoded.bytes().to_vec();
        reflow_production_functions(&mut functions, physical)?;
        let new = &functions[function_index].blocks[block_index].instructions[instruction_index];
        let current_bytes = total_bytes(&functions)?;
        if previous_bytes.checked_sub(current_bytes) != Some(4) {
            return Err(OptimizedX86BranchRelaxationError::NonDecreasingByteMeasure);
        }
        let output = revision_identity(roots, &functions);
        let old_displacement = old
            .branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                old.instruction,
            ))?
            .byte_displacement;
        let new_displacement = new
            .branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                new.instruction,
            ))?
            .byte_displacement;
        actions.push(TerminalX86BranchRelaxationAction {
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
        });
    }
    finish_artifact(source, budget, usage, attempts, actions, functions)
}

fn replay_relaxation(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let roots = RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    };
    let mut functions = source.functions().to_vec();
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();
    loop {
        replay_charge(
            &mut usage.iterations,
            budget.iterations(),
            TerminalX86BranchRelaxationWorkAxis::Iterations,
        )?;
        let iteration = usage.iterations;
        let input = revision_identity(roots, &functions);
        let before = total_bytes(&functions)?;
        let locations = ordered_branch_locations(&functions);
        let mut chosen = None;
        for (function_index, block_index, instruction_index) in locations {
            replay_charge(
                &mut usage.rule_evaluations,
                budget.rule_evaluations(),
                TerminalX86BranchRelaxationWorkAxis::RuleEvaluations,
            )?;
            let row =
                &functions[function_index].blocks[block_index].instructions[instruction_index];
            let branch =
                row.branch
                    .as_deref()
                    .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                        row.instruction,
                    ))?;
            let (outcome, displacement) = replay_inspect_branch(
                &functions[function_index],
                block_index,
                instruction_index,
                physical,
            )?;
            attempts.push(TerminalX86BranchRelaxationAttempt {
                iteration,
                input,
                instruction: row.instruction,
                offset: row.offset,
                byte_displacement: branch.byte_displacement,
                encoded_bytes: u8::try_from(row.bytes.len())
                    .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                outcome,
            });
            if outcome == TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation {
                replay_charge(
                    &mut usage.candidates,
                    budget.candidates(),
                    TerminalX86BranchRelaxationWorkAxis::Candidates,
                )?;
                replay_charge(
                    &mut usage.validation_steps,
                    budget.validation_steps(),
                    TerminalX86BranchRelaxationWorkAxis::ValidationSteps,
                )?;
                let displacement = displacement.ok_or(
                    OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction),
                )?;
                chosen = Some((function_index, block_index, instruction_index, displacement));
                break;
            }
        }
        let Some((function_index, block_index, instruction_index, displacement)) = chosen else {
            break;
        };
        replay_charge(
            &mut usage.commits,
            budget.commits(),
            TerminalX86BranchRelaxationWorkAxis::Commits,
        )?;
        let old =
            functions[function_index].blocks[block_index].instructions[instruction_index].clone();
        let bytes = [0x75, displacement as i8 as u8];
        validate_x86_64_terminal_selected_short_nonzero_branch_form(
            physical,
            old.alternative,
            displacement,
            &bytes,
        )
        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
        functions[function_index].blocks[block_index].instructions[instruction_index].bytes =
            bytes.to_vec();
        reflow_replay_functions(&mut functions, physical)?;
        if before.checked_sub(total_bytes(&functions)?) != Some(4) {
            return Err(OptimizedX86BranchRelaxationError::NonDecreasingByteMeasure);
        }
        let new = &functions[function_index].blocks[block_index].instructions[instruction_index];
        let output = revision_identity(roots, &functions);
        actions.push(TerminalX86BranchRelaxationAction {
            iteration,
            input,
            output,
            instruction: old.instruction,
            old_offset: old.offset,
            new_offset: new.offset,
            old_displacement: old
                .branch
                .as_deref()
                .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                    old.instruction,
                ))?
                .byte_displacement,
            new_displacement: new
                .branch
                .as_deref()
                .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                    new.instruction,
                ))?
                .byte_displacement,
            old_bytes: old.bytes,
            new_bytes: new.bytes.clone(),
        });
    }
    finish_artifact(source, budget, usage, attempts, actions, functions)
}

fn finish_artifact(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    attempts: Vec<TerminalX86BranchRelaxationAttempt>,
    actions: Vec<TerminalX86BranchRelaxationAction>,
    functions: Vec<TerminalResolvedSelectedFunctionLayout>,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let roots = RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    };
    let output_revision = revision_identity(roots, &functions);
    let layout = source.with_replayed_functions(functions);
    let output = layout.identity();
    let policy = TerminalX86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1;
    let identity = artifact_identity(
        roots,
        policy,
        budget,
        usage,
        output,
        output_revision,
        &attempts,
        &actions,
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
        usage,
        output,
        output_revision,
        identity,
        attempts,
        actions,
        layout,
    })
}

fn inspect_production_branch(
    function: &TerminalResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<
    (TerminalX86BranchRelaxationAttemptOutcome, Option<i64>),
    OptimizedX86BranchRelaxationError,
> {
    let row = &function.blocks[block_index].instructions[instruction_index];
    let branch =
        row.branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ))?;
    if row.bytes.len() == 2 && row.bytes[0] == 0x75 {
        validate_x86_64_terminal_selected_short_nonzero_branch_form(
            physical,
            row.alternative,
            branch.byte_displacement,
            &row.bytes,
        )
        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
        return Ok((
            TerminalX86BranchRelaxationAttemptOutcome::AlreadyShort,
            None,
        ));
    }
    validate_x86_64_terminal_selected_nonzero_branch_form(
        physical,
        row.alternative,
        branch.byte_displacement,
        &row.bytes,
    )
    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
    let displacement = prospective_short_displacement(function, row, branch.when_nonzero_block)?;
    if i8::try_from(displacement).is_ok() {
        Ok((
            TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(displacement),
        ))
    } else {
        Ok((
            TerminalX86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
            None,
        ))
    }
}

fn replay_inspect_branch(
    function: &TerminalResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<
    (TerminalX86BranchRelaxationAttemptOutcome, Option<i64>),
    OptimizedX86BranchRelaxationError,
> {
    let row = function
        .blocks
        .get(block_index)
        .and_then(|block| block.instructions.get(instruction_index))
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    let branch =
        row.branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ))?;
    match row.bytes.as_slice() {
        [0x75, displacement] => {
            let decoded = i64::from(*displacement as i8);
            if decoded != branch.byte_displacement {
                return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                    row.instruction,
                ));
            }
            validate_x86_64_terminal_selected_short_nonzero_branch_form(
                physical,
                row.alternative,
                decoded,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            Ok((
                TerminalX86BranchRelaxationAttemptOutcome::AlreadyShort,
                None,
            ))
        }
        [0x0f, 0x85, ..] if row.bytes.len() == 6 => {
            validate_x86_64_terminal_selected_nonzero_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            let displacement =
                prospective_short_displacement(function, row, branch.when_nonzero_block)?;
            if (-128..=127).contains(&displacement) {
                Ok((
                    TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                    Some(displacement),
                ))
            } else {
                Ok((
                    TerminalX86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
                    None,
                ))
            }
        }
        _ => Err(OptimizedX86BranchRelaxationError::MalformedBranch(
            row.instruction,
        )),
    }
}

fn prospective_short_displacement(
    function: &TerminalResolvedSelectedFunctionLayout,
    row: &TerminalResolvedSelectedFormRow,
    target: TerminalSelectedBlockId,
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
    checked_delta(
        shifted_target,
        row.offset
            .checked_add(2)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?,
    )
}

fn reflow_production_functions(
    functions: &mut [TerminalResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let offsets = assign_dense_offsets(function)?;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_mut() else {
                    continue;
                };
                rewrite_branch_offsets(
                    branch,
                    row.offset,
                    row.bytes.len(),
                    &offsets,
                    row.instruction,
                )?;
                let encoded = if row.bytes.len() == 2 {
                    encode_x86_64_terminal_selected_short_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                    )
                } else {
                    encode_x86_64_terminal_selected_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                    )
                }
                .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
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

fn reflow_replay_functions(
    functions: &mut [TerminalResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let mut next = 0_u64;
        let mut offsets = BTreeMap::new();
        for block in &mut function.blocks {
            block.offset = next;
            offsets.insert(block.block, next);
            let mut local = next;
            for row in &mut block.instructions {
                row.offset = local;
                local = local
                    .checked_add(
                        u64::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                    )
                    .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
            }
            block.byte_count = local - next;
            next = local;
        }
        function.byte_count = next;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_mut() else {
                    continue;
                };
                let nonzero = offsets.get(&branch.when_nonzero_block).copied().ok_or(
                    OptimizedX86BranchRelaxationError::MissingTargetBlock(
                        branch.when_nonzero_block,
                    ),
                )?;
                let zero = offsets.get(&branch.when_zero_block).copied().ok_or(
                    OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_zero_block),
                )?;
                let end = row
                    .offset
                    .checked_add(
                        u64::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                    )
                    .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
                if zero != end {
                    return Err(
                        OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(
                            row.instruction,
                        ),
                    );
                }
                branch.when_nonzero_offset = nonzero;
                branch.when_zero_offset = zero;
                branch.byte_displacement = checked_delta(nonzero, end)?;
                if row.bytes.len() == 2 {
                    let bytes = [0x75, branch.byte_displacement as i8 as u8];
                    let decoded = validate_x86_64_terminal_selected_short_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        &bytes,
                    )
                    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                    if decoded.footprint().encoded != branch.decoded_effects {
                        return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    row.bytes = bytes.to_vec();
                } else {
                    let mut bytes = vec![0x0f, 0x85];
                    let displacement = i32::try_from(branch.byte_displacement).map_err(|_| {
                        OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction)
                    })?;
                    bytes.extend_from_slice(&displacement.to_le_bytes());
                    let decoded = validate_x86_64_terminal_selected_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        &bytes,
                    )
                    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                    if decoded.footprint().encoded != branch.decoded_effects {
                        return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    row.bytes = bytes;
                }
            }
        }
    }
    Ok(())
}

fn assign_dense_offsets(
    function: &mut TerminalResolvedSelectedFunctionLayout,
) -> Result<BTreeMap<TerminalSelectedBlockId, u64>, OptimizedX86BranchRelaxationError> {
    let mut offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in &mut function.blocks {
        block.offset = offset;
        offsets.insert(block.block, offset);
        let start = offset;
        for row in &mut block.instructions {
            row.offset = offset;
            offset = offset
                .checked_add(
                    u64::try_from(row.bytes.len())
                        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                )
                .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
        }
        block.byte_count = offset - start;
    }
    function.byte_count = offset;
    Ok(offsets)
}

fn rewrite_branch_offsets(
    branch: &mut crate::TerminalResolvedConditionalBranchEvidence,
    instruction_offset: u64,
    instruction_size: usize,
    offsets: &BTreeMap<TerminalSelectedBlockId, u64>,
    instruction: TerminalSelectedInstructionId,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let nonzero = *offsets.get(&branch.when_nonzero_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_nonzero_block),
    )?;
    let zero = *offsets.get(&branch.when_zero_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_zero_block),
    )?;
    let end = instruction_offset
        .checked_add(
            u64::try_from(instruction_size)
                .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
        )
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    if zero != end {
        return Err(OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(instruction));
    }
    branch.when_nonzero_offset = nonzero;
    branch.when_zero_offset = zero;
    branch.byte_displacement = checked_delta(nonzero, end)?;
    Ok(())
}

fn ordered_branch_locations(
    functions: &[TerminalResolvedSelectedFunctionLayout],
) -> Vec<(usize, usize, usize)> {
    let mut locations = Vec::new();
    for (function_index, function) in functions.iter().enumerate() {
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, row) in block.instructions.iter().enumerate() {
                if row.branch.is_some() {
                    locations.push((function_index, block_index, instruction_index));
                }
            }
        }
    }
    locations
}

fn total_bytes(
    functions: &[TerminalResolvedSelectedFunctionLayout],
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
    axis: TerminalX86BranchRelaxationWorkAxis,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(OptimizedX86BranchRelaxationError::BudgetExceeded(axis))?;
    if *usage > limit {
        return Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis));
    }
    Ok(())
}

fn replay_charge(
    usage: &mut u64,
    limit: u64,
    axis: TerminalX86BranchRelaxationWorkAxis,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let next = usage
        .checked_add(1)
        .ok_or(OptimizedX86BranchRelaxationError::BudgetExceeded(axis))?;
    if next > limit {
        return Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis));
    }
    *usage = next;
    Ok(())
}

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedX86BranchRelaxationError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)
}

#[derive(Clone, Copy)]
struct RevisionRoots {
    source: TerminalResolvedSelectedFormLayoutIdentity,
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pre_layout: crate::TerminalSelectedFormEncodingIdentity,
    target: NativeTarget,
}

fn revision_identity(
    roots: RevisionRoots,
    functions: &[TerminalResolvedSelectedFunctionLayout],
) -> TerminalX86BranchRelaxationRevisionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(REVISION_SCHEMA);
    encode_roots(&mut hasher, roots);
    encode_functions(&mut hasher, functions);
    TerminalX86BranchRelaxationRevisionIdentity(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
fn artifact_identity(
    roots: RevisionRoots,
    policy: TerminalX86BranchRelaxationPolicy,
    budget: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    output: TerminalResolvedSelectedFormLayoutIdentity,
    output_revision: TerminalX86BranchRelaxationRevisionIdentity,
    attempts: &[TerminalX86BranchRelaxationAttempt],
    actions: &[TerminalX86BranchRelaxationAction],
    functions: &[TerminalResolvedSelectedFunctionLayout],
) -> TerminalX86BranchRelaxationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(RELAXATION_SCHEMA);
    encode_roots(&mut hasher, roots);
    hasher.update([match policy {
        TerminalX86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1 => 0,
    }]);
    hasher.update(budget.encode());
    hasher.update(usage.encode());
    hasher.update(output.bytes());
    hasher.update(output_revision.bytes());
    hasher.update((attempts.len() as u64).to_le_bytes());
    for attempt in attempts {
        hasher.update(attempt.iteration.to_le_bytes());
        hasher.update(attempt.input.bytes());
        hasher.update(attempt.instruction.0.to_le_bytes());
        hasher.update(attempt.offset.to_le_bytes());
        hasher.update(attempt.byte_displacement.to_le_bytes());
        hasher.update([attempt.encoded_bytes]);
        hasher.update([match attempt.outcome {
            TerminalX86BranchRelaxationAttemptOutcome::AlreadyShort => 0,
            TerminalX86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8 => 1,
            TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation => 2,
        }]);
    }
    hasher.update((actions.len() as u64).to_le_bytes());
    for action in actions {
        hasher.update(action.iteration.to_le_bytes());
        hasher.update(action.input.bytes());
        hasher.update(action.output.bytes());
        hasher.update(action.instruction.0.to_le_bytes());
        hasher.update(action.old_offset.to_le_bytes());
        hasher.update(action.new_offset.to_le_bytes());
        hasher.update(action.old_displacement.to_le_bytes());
        hasher.update(action.new_displacement.to_le_bytes());
        hasher.update((action.old_bytes.len() as u64).to_le_bytes());
        hasher.update(&action.old_bytes);
        hasher.update((action.new_bytes.len() as u64).to_le_bytes());
        hasher.update(&action.new_bytes);
    }
    encode_functions(&mut hasher, functions);
    TerminalX86BranchRelaxationIdentity(hasher.finalize().into())
}

fn encode_roots(hasher: &mut Sha256, roots: RevisionRoots) {
    hasher.update(roots.source.bytes());
    hasher.update(roots.selected.bytes());
    hasher.update(roots.machine.bytes());
    hasher.update(roots.pre_layout.bytes());
    hasher.update([match roots.target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }]);
    hasher.update([match roots.target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    }]);
    hasher.update((roots.target.pointer_size as u64).to_le_bytes());
    hasher.update((roots.target.pointer_alignment as u64).to_le_bytes());
}

fn encode_functions(hasher: &mut Sha256, functions: &[TerminalResolvedSelectedFunctionLayout]) {
    hasher.update((functions.len() as u64).to_le_bytes());
    for function in functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.byte_count.to_le_bytes());
        hasher.update((function.blocks.len() as u64).to_le_bytes());
        for block in &function.blocks {
            hasher.update(block.block.0.to_le_bytes());
            hasher.update(block.offset.to_le_bytes());
            hasher.update(block.byte_count.to_le_bytes());
            hasher.update((block.instructions.len() as u64).to_le_bytes());
            for row in &block.instructions {
                hasher.update(row.instruction.0.to_le_bytes());
                encode_alternative(hasher, row.alternative);
                hasher.update(row.offset.to_le_bytes());
                hasher.update((row.bytes.len() as u64).to_le_bytes());
                hasher.update(&row.bytes);
                match &row.branch {
                    None => hasher.update([0]),
                    Some(branch) => {
                        hasher.update([1]);
                        hasher.update(branch.source_block.0.to_le_bytes());
                        hasher.update(branch.when_nonzero_edge.get().to_le_bytes());
                        hasher.update(branch.when_nonzero_block.0.to_le_bytes());
                        hasher.update(branch.when_nonzero_offset.to_le_bytes());
                        hasher.update(branch.when_zero_edge.get().to_le_bytes());
                        hasher.update(branch.when_zero_block.0.to_le_bytes());
                        hasher.update(branch.when_zero_offset.to_le_bytes());
                        hasher.update(branch.byte_displacement.to_le_bytes());
                        encode_effects(hasher, &branch.decoded_effects);
                    }
                }
            }
        }
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: TerminalMachineAlternativeKey) {
    hasher.update([match alternative.family {
        TerminalMachineAlternativeFamily::CompareI64Zero => 0,
        TerminalMachineAlternativeFamily::MaterializeI64 => 1,
        TerminalMachineAlternativeFamily::CopyI64 => 2,
        TerminalMachineAlternativeFamily::ExactAddI64 => 3,
        TerminalMachineAlternativeFamily::ExactAddI64Immediate => 4,
        TerminalMachineAlternativeFamily::ExactSubtractI64 => 5,
        TerminalMachineAlternativeFamily::ConditionalBranchNonZero => 6,
        TerminalMachineAlternativeFamily::ReturnI64 => 7,
        TerminalMachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        TerminalMachineAlternativeFamily::ReturnUnit => 9,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_effects(hasher: &mut Sha256, effects: &TerminalMachineEncodedEffects) {
    encode_u16s(hasher, &effects.external_operand_reads);
    encode_u16s(hasher, &effects.external_operand_writes);
    encode_units(hasher, &effects.implicit_unit_uses);
    encode_units(hasher, &effects.implicit_unit_defs);
    encode_units(hasher, &effects.implicit_unit_clobbers);
    match effects.memory {
        TerminalMachineEncodedMemoryEffect::NoneV1 => hasher.update([0]),
        TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        TerminalMachineEncodedStackEffect::UnchangedV1 => hasher.update([0]),
        TerminalMachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        TerminalMachineEncodedTrapBehavior::NeverV1 => 0,
        TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        TerminalMachineEncodedControlEffect::FallThroughV1 => hasher.update([0]),
        TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1 => hasher.update([1]),
        TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1 => hasher.update([2]),
        TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
    }
}

fn encode_u16s(hasher: &mut Sha256, values: &[u16]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, values: &[omega_register_model::RegisterUnitId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use omega_register_model::validate_physical_register_model;
    use omega_terminal_isa_x86_64::x86_64_physical_register_model;
    use psi_core::{EdgeId, MachineId};

    use crate::TerminalResolvedSelectedBlockLayout;

    use super::*;

    fn physical() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(x86_64_physical_register_model()).unwrap()
    }

    fn alternative() -> TerminalMachineAlternativeKey {
        TerminalMachineAlternativeKey {
            family: TerminalMachineAlternativeFamily::ConditionalBranchNonZero,
            variant: 0,
        }
    }

    fn function(zero_arm_bytes: usize) -> TerminalResolvedSelectedFunctionLayout {
        let physical = physical();
        let displacement = i64::try_from(zero_arm_bytes).unwrap();
        let near = encode_x86_64_terminal_selected_nonzero_branch_form(
            &physical,
            alternative(),
            displacement,
        )
        .unwrap();
        let entry = TerminalSelectedBlockId(0);
        let zero = TerminalSelectedBlockId(1);
        let nonzero = TerminalSelectedBlockId(2);
        let zero_offset = 6;
        let nonzero_offset = zero_offset + u64::try_from(zero_arm_bytes).unwrap();
        TerminalResolvedSelectedFunctionLayout {
            machine: MachineId::new(1).unwrap(),
            byte_count: nonzero_offset + 1,
            blocks: vec![
                TerminalResolvedSelectedBlockLayout {
                    block: entry,
                    offset: 0,
                    byte_count: 6,
                    instructions: vec![TerminalResolvedSelectedFormRow {
                        instruction: TerminalSelectedInstructionId(0),
                        alternative: alternative(),
                        offset: 0,
                        bytes: near.bytes().to_vec(),
                        branch: Some(Box::new(crate::TerminalResolvedConditionalBranchEvidence {
                            source_block: entry,
                            when_nonzero_edge: EdgeId::new(1).unwrap(),
                            when_nonzero_block: nonzero,
                            when_nonzero_offset: nonzero_offset,
                            when_zero_edge: EdgeId::new(2).unwrap(),
                            when_zero_block: zero,
                            when_zero_offset: zero_offset,
                            byte_displacement: displacement,
                            decoded_register_reads: vec![],
                            decoded_effects: near.footprint().encoded.clone(),
                        })),
                    }],
                },
                TerminalResolvedSelectedBlockLayout {
                    block: zero,
                    offset: zero_offset,
                    byte_count: u64::try_from(zero_arm_bytes).unwrap(),
                    instructions: vec![TerminalResolvedSelectedFormRow {
                        instruction: TerminalSelectedInstructionId(1),
                        alternative: TerminalMachineAlternativeKey {
                            family: TerminalMachineAlternativeFamily::ReturnI64,
                            variant: 0,
                        },
                        offset: zero_offset,
                        bytes: vec![0x90; zero_arm_bytes],
                        branch: None,
                    }],
                },
                TerminalResolvedSelectedBlockLayout {
                    block: nonzero,
                    offset: nonzero_offset,
                    byte_count: 1,
                    instructions: vec![TerminalResolvedSelectedFormRow {
                        instruction: TerminalSelectedInstructionId(2),
                        alternative: TerminalMachineAlternativeKey {
                            family: TerminalMachineAlternativeFamily::ReturnI64,
                            variant: 0,
                        },
                        offset: nonzero_offset,
                        bytes: vec![0xc3],
                        branch: None,
                    }],
                },
            ],
        }
    }

    #[test]
    fn eligible_near_branch_shrinks_and_both_reflow_implementations_agree() {
        let physical = physical();
        let source = function(127);
        assert_eq!(
            inspect_production_branch(&source, 0, 0, &physical).unwrap(),
            (
                TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                Some(127),
            )
        );
        assert_eq!(
            replay_inspect_branch(&source, 0, 0, &physical).unwrap(),
            (
                TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                Some(127),
            )
        );

        let short = encode_x86_64_terminal_selected_short_nonzero_branch_form(
            &physical,
            alternative(),
            127,
        )
        .unwrap();
        let mut produced = vec![source.clone()];
        produced[0].blocks[0].instructions[0].bytes = short.bytes().to_vec();
        let mut replayed = produced.clone();
        reflow_production_functions(&mut produced, &physical).unwrap();
        reflow_replay_functions(&mut replayed, &physical).unwrap();
        assert_eq!(produced, replayed);
        assert_eq!(source.byte_count - produced[0].byte_count, 4);
        assert_eq!(produced[0].blocks[0].instructions[0].bytes, [0x75, 0x7f]);
        assert_eq!(produced[0].blocks[1].offset, 2);
        assert_eq!(produced[0].blocks[2].offset, 129);
        assert_eq!(
            produced[0].blocks[0].instructions[0]
                .branch
                .as_deref()
                .unwrap()
                .byte_displacement,
            127
        );
        assert_eq!(
            inspect_production_branch(&produced[0], 0, 0, &physical).unwrap(),
            (
                TerminalX86BranchRelaxationAttemptOutcome::AlreadyShort,
                None,
            )
        );
        assert_eq!(
            replay_inspect_branch(&produced[0], 0, 0, &physical).unwrap(),
            (
                TerminalX86BranchRelaxationAttemptOutcome::AlreadyShort,
                None,
            )
        );

        let fixed_point = produced.clone();
        reflow_production_functions(&mut produced, &physical).unwrap();
        reflow_replay_functions(&mut replayed, &physical).unwrap();
        assert_eq!(produced, fixed_point);
        assert_eq!(replayed, fixed_point);
    }

    #[test]
    fn out_of_range_near_branch_is_a_verified_no_change_attempt() {
        let physical = physical();
        let source = function(128);
        assert_eq!(
            inspect_production_branch(&source, 0, 0, &physical).unwrap(),
            (
                TerminalX86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
                None,
            )
        );
        assert_eq!(
            replay_inspect_branch(&source, 0, 0, &physical).unwrap(),
            (
                TerminalX86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
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
            replay_inspect_branch(&source, 0, 0, &physical),
            Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                TerminalSelectedInstructionId(0)
            ))
        ));

        let mut usage = 0;
        assert_eq!(
            charge(&mut usage, 0, TerminalX86BranchRelaxationWorkAxis::Commits),
            Err(OptimizedX86BranchRelaxationError::BudgetExceeded(
                TerminalX86BranchRelaxationWorkAxis::Commits
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

    #[test]
    fn corrupted_action_changes_identity_and_is_rejected_by_replay_comparison() {
        let roots = RevisionRoots {
            source: TerminalResolvedSelectedFormLayoutIdentity::from_bytes([1; 32]),
            selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity::from_bytes([2; 32]),
            machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity::from_bytes([3; 32]),
            pre_layout: crate::TerminalSelectedFormEncodingIdentity::from_bytes([4; 32]),
            target: NativeTarget::linux_x64(),
        };
        let functions = vec![function(1)];
        let input = revision_identity(roots, &functions);
        let action = TerminalX86BranchRelaxationAction {
            iteration: 1,
            input,
            output: TerminalX86BranchRelaxationRevisionIdentity::from_bytes([5; 32]),
            instruction: TerminalSelectedInstructionId(0),
            old_offset: 0,
            new_offset: 0,
            old_displacement: 1,
            new_displacement: 1,
            old_bytes: vec![0x0f, 0x85, 1, 0, 0, 0],
            new_bytes: vec![0x75, 1],
        };
        let attempts = vec![TerminalX86BranchRelaxationAttempt {
            iteration: 1,
            input,
            instruction: TerminalSelectedInstructionId(0),
            offset: 0,
            byte_displacement: 1,
            encoded_bytes: 6,
            outcome: TerminalX86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
        }];
        let budget = OptimizationWorkBudget::new(8, 8, 8, 8, 8).unwrap();
        let usage = OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 1,
            validation_steps: 1,
            commits: 1,
            iterations: 2,
        };
        let output = TerminalResolvedSelectedFormLayoutIdentity::from_bytes([6; 32]);
        let output_revision = TerminalX86BranchRelaxationRevisionIdentity::from_bytes([7; 32]);
        let identity = artifact_identity(
            roots,
            TerminalX86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1,
            budget,
            usage,
            output,
            output_revision,
            &attempts,
            std::slice::from_ref(&action),
            &functions,
        );
        let mut corrupted = action.clone();
        corrupted.new_bytes = vec![0x74, 1];
        let corrupted_identity = artifact_identity(
            roots,
            TerminalX86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1,
            budget,
            usage,
            output,
            output_revision,
            &attempts,
            std::slice::from_ref(&corrupted),
            &functions,
        );
        assert_ne!(identity, corrupted_identity);
        assert_eq!(
            compare_replayed_action_evidence(
                &attempts,
                std::slice::from_ref(&corrupted),
                &attempts,
                std::slice::from_ref(&action),
            ),
            Err(OptimizedX86BranchRelaxationError::ArtifactMismatch)
        );
    }
}
