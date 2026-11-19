//! Validate relaxation records independently of the layout optimizer.

use isa_x86_64::X86_64SelectedFormEncodingError;
use machine_code::ResolvedMachineLayout;
use machine_code::layout::X86BranchRelaxationRecord;
use machine_code::layout::evidence::X86BranchRelaxationPolicy;
use machine_code::layout::evidence::relaxation_identity::{
    RevisionRoots, artifact_identity, revision_identity,
};
use optimization_core::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use target::{Architecture, NativeTarget};

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

pub fn validate_optimized_x86_branch_relaxation(
    physical: &ValidatedPhysicalRegisterModel,
    source: &ResolvedMachineLayout,
    artifact: &X86BranchRelaxationRecord,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    validate_roots(source, physical)?;
    validate_current_x86_branch_relaxation(physical, source.roots(), source.target(), artifact)
}

pub fn validate_current_x86_branch_relaxation(
    physical: &ValidatedPhysicalRegisterModel,
    source: machine_code::ResolvedLayoutRoots,
    target: NativeTarget,
    artifact: &X86BranchRelaxationRecord,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    ensure_x86_target(target, physical)?;
    if artifact.source != source.identity
        || artifact.selected != source.selected
        || artifact.machine != source.machine
        || artifact.pre_layout != source.pre_layout
        || artifact.target != target
        || artifact.policy != X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1
        || !artifact.usage.within(artifact.budget)
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    let roots = RevisionRoots {
        source: source.identity,
        selected: source.selected,
        machine: source.machine,
        pre_layout: source.pre_layout,
        target,
    };
    if artifact.output != artifact.layout.identity()
        || artifact.output_revision != revision_identity(roots, artifact.functions())
        || artifact.identity
            != artifact_identity(
                roots,
                artifact.policy,
                artifact.budget,
                artifact.usage,
                artifact.output,
                artifact.output_revision,
                &artifact.attempts,
                &artifact.actions,
                artifact.functions(),
            )
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

pub fn validate_roots(
    source: &ResolvedMachineLayout,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    ensure_x86_target(source.target(), physical)
}

pub fn ensure_x86_target(
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

pub type FunctionRelativeLayoutRuleCatalogEntry = OptimizationCatalogDescriptor<Architecture>;

pub const FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG: [FunctionRelativeLayoutRuleCatalogEntry; 1] =
    [FunctionRelativeLayoutRuleCatalogEntry::new(
        Optimization::X86RelaxConditionalBranchesToRel8V1,
        Architecture::X86_64,
    )];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeLayoutCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
    UnsupportedTarget {
        optimization: Optimization, // Optimization rule rejected for the target architecture.
        required: Architecture,     // Architecture required by the catalog entry.
        actual: Architecture,       // Architecture supplied by the compilation target.
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLayoutOptimizationError {
    Catalog(FunctionRelativeLayoutCatalogError),
    Relaxation(OptimizedX86BranchRelaxationError),
    CurrentProgramMismatch,
    SelectionMismatch,
}

impl std::fmt::Display for ResolvedLayoutOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "resolved-layout optimization failed: {self:?}")
    }
}

impl std::error::Error for ResolvedLayoutOptimizationError {}

pub fn relaxation_roots(
    relaxation: &X86BranchRelaxationRecord,
) -> machine_code::X86BranchRelaxationRoots {
    machine_code::X86BranchRelaxationRoots {
        identity: relaxation.identity(),
        source: relaxation.source(),
        output: relaxation.output(),
    }
}

/// Replay the run from the original baseline layout: the current form and its
/// relaxation trace must be what the selections and budget produce from it.
pub fn validate_resolved_layout_optimization(
    physical: &ValidatedPhysicalRegisterModel,
    baseline: &ResolvedMachineLayout,
    selections: &OptimizationPhaseSelections,
    current: &machine_code::OptimizedMachineLayout,
    relaxation: Option<&X86BranchRelaxationRecord>,
) -> Result<(), ResolvedLayoutOptimizationError> {
    if &current.selections != selections {
        return Err(ResolvedLayoutOptimizationError::SelectionMismatch);
    }
    if current.baseline != baseline.roots()
        || current.relaxation != relaxation.map(relaxation_roots)
    {
        return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
    }
    let enabled = x86_rel8_selected(selections, baseline.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    match (enabled, relaxation) {
        (false, None) => {
            if current.layout() != baseline {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        (true, Some(relaxation)) => {
            validate_optimized_x86_branch_relaxation(physical, baseline, relaxation)
                .map_err(ResolvedLayoutOptimizationError::Relaxation)?;
            if current.budget != relaxation.budget() || current.layout() != relaxation.layout() {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        _ => return Err(ResolvedLayoutOptimizationError::SelectionMismatch),
    }
    Ok(())
}

/// Check phase, budget and result custody from the current form and its
/// relaxation trace alone. Earlier layout rows are required only by
/// `validate_resolved_layout_optimization`.
pub fn validate_current_layout_optimization(
    current: &machine_code::OptimizedMachineLayout,
    relaxation: Option<&X86BranchRelaxationRecord>,
) -> Result<(), ResolvedLayoutOptimizationError> {
    let program = &current.program;
    let facts = &program.context.publication;
    let selections = facts
        .selections()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if current.selections != selections
        || current.budget != facts.budget()
        || current.relaxation != relaxation.map(relaxation_roots)
        || current.baseline.selected != program.context.machine.selected
        || current.baseline.machine != program.context.machine.identity
        || current.baseline.pre_layout != program.encoding.identity()
    {
        return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
    }
    let enabled = x86_rel8_selected(&selections, program.context.target.architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    match (enabled, relaxation) {
        (false, None) if current.layout().identity() == current.baseline.identity => Ok(()),
        (true, Some(relaxation)) => {
            validate_current_x86_branch_relaxation(
                program.context.physical(),
                current.baseline,
                program.context.target,
                relaxation,
            )
            .map_err(ResolvedLayoutOptimizationError::Relaxation)?;
            if relaxation.layout() != current.layout() || relaxation.budget() != current.budget {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
            Ok(())
        }
        _ => Err(ResolvedLayoutOptimizationError::SelectionMismatch),
    }
}

pub fn x86_rel8_selected(
    selections: &OptimizationPhaseSelections,
    architecture: Architecture,
) -> Result<bool, FunctionRelativeLayoutCatalogError> {
    let phase = selections
        .require_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
        .map_err(FunctionRelativeLayoutCatalogError::WrongPhase)?;
    match phase.as_slice() {
        [] => Ok(false),
        [Optimization::X86RelaxConditionalBranchesToRel8V1] => {
            let descriptor = FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG[0];
            let required = *descriptor.payload();
            if architecture != required {
                return Err(FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                    optimization: descriptor.optimization(),
                    required,
                    actual: architecture,
                });
            }
            Ok(true)
        }
        [unsupported] => Err(FunctionRelativeLayoutCatalogError::UnsupportedSelection(
            *unsupported,
        )),
        _ => Err(FunctionRelativeLayoutCatalogError::UnsupportedComposition),
    }
}
