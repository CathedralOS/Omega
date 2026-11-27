//! Check layout optimization records against their independently retained inputs.
use machine_code::ResolvedMachineLayout;
use machine_code::layout::X86BranchRelaxationRecord;
use machine_code::layout::evidence::X86BranchRelaxationPolicy;
use machine_code::layout::evidence::relaxation_identity::{
    RevisionRoots, artifact_identity, revision_identity,
};
use machine_code::layout::relaxation_policy::{
    OptimizedX86BranchRelaxationError, ResolvedLayoutOptimizationError, ensure_x86_target,
    validate_roots, x86_rel8_selected,
};
use optimization_core::{OptimizationExecutionPhase, OptimizationPhaseSelections};
use register_model::ValidatedPhysicalRegisterModel;
use target::NativeTarget;
pub(super) fn validate_optimized_x86_branch_relaxation(
    physical: &ValidatedPhysicalRegisterModel,
    source: &ResolvedMachineLayout,
    artifact: &X86BranchRelaxationRecord,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    validate_roots(source, physical)?;
    validate_current_x86_branch_relaxation(physical, &source.roots(), source.target(), artifact)?;
    // The record must start from this baseline's rows. Every attempt in the
    // first sweep names the revision of the rows it inspected; with no
    // conditional branch to inspect, the sweep leaves the rows unchanged.
    let started_from_source = match artifact.attempts().first() {
        None => artifact.functions() == source.functions(),
        Some(first) => {
            first.iteration == 1
                && first.input
                    == revision_identity(
                        revision_roots(&source.roots(), source.target()),
                        source.functions(),
                    )
        }
    };
    if !started_from_source {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

fn revision_roots(
    source: &machine_code::ResolvedLayoutRoots,
    target: NativeTarget,
) -> RevisionRoots {
    RevisionRoots {
        selected: source.selected,
        pre_layout: source.pre_layout,
        target,
    }
}

fn validate_current_x86_branch_relaxation(
    physical: &ValidatedPhysicalRegisterModel,
    source: &machine_code::ResolvedLayoutRoots,
    target: NativeTarget,
    artifact: &X86BranchRelaxationRecord,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    ensure_x86_target(target, physical)?;
    if artifact.selected != source.selected
        || artifact.machine != source.machine
        || artifact.pre_layout != source.pre_layout
        || artifact.target != target
        || artifact.policy != X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1
        || !artifact.usage.within(artifact.budget)
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    let roots = revision_roots(source, target);
    if artifact.output_revision != revision_identity(roots, artifact.functions())
        || artifact.identity
            != artifact_identity(
                roots,
                artifact.policy,
                artifact.budget,
                artifact.usage,
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
        || current.relaxation != relaxation.map(X86BranchRelaxationRecord::identity)
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
        || current.relaxation != relaxation.map(X86BranchRelaxationRecord::identity)
        || current.baseline.selected != program.context.machine.selected
        || current.baseline.machine != program.context.plan
        || current.baseline.pre_layout != program.encoding.identity()
    {
        return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
    }
    let enabled = x86_rel8_selected(&selections, program.context.target.architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    match (enabled, relaxation) {
        // Without relaxation the current layout is the baseline, so it keeps
        // the baseline's roots.
        (false, None) if current.layout().roots() == current.baseline => Ok(()),
        (true, Some(relaxation)) => {
            validate_current_x86_branch_relaxation(
                program.context.physical(),
                &current.baseline,
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

/// A relaxation may shorten a conditional and move instruction addresses.
/// Its input has already established the instruction identities, edges and ordinary bytes.
pub(super) fn validate_layout_transition(
    baseline: &ResolvedMachineLayout,
    output: &ResolvedMachineLayout,
    controls: &[machine_code::EncodedControlIndex],
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let mismatch = OptimizedX86BranchRelaxationError::ArtifactMismatch;
    if output.selected != baseline.selected
        || output.machine != baseline.machine
        || output.pre_layout != baseline.pre_layout
        || output.policy != baseline.policy
        || output.target != baseline.target
        || output.target.architecture != target::Architecture::X86_64
        || output.functions.len() != baseline.functions.len()
        || output.functions.len() != controls.len()
    {
        return Err(mismatch);
    }
    for ((before, after), forms) in baseline
        .functions
        .iter()
        .zip(&output.functions)
        .zip(controls)
    {
        if before.machine != after.machine || before.blocks.len() != after.blocks.len() {
            return Err(mismatch);
        }
        let offsets: std::collections::BTreeMap<_, _> = after
            .blocks
            .iter()
            .map(|block| (block.block, block.offset))
            .collect();
        let mut offset = 0_u64;
        for (original, block) in before.blocks.iter().zip(&after.blocks) {
            if original.block != block.block
                || block.offset != offset
                || original.instructions.len() != block.instructions.len()
            {
                return Err(mismatch);
            }
            for (source, row) in original.instructions.iter().zip(&block.instructions) {
                let end = offset
                    .checked_add(row.bytes.len() as u64)
                    .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
                if row.offset != offset
                    || row.instruction != source.instruction
                    || row.internal_machine_fixup != source.internal_machine_fixup
                    || row.normalized_foreign_call_fixup != source.normalized_foreign_call_fixup
                    || row.image_address_fixup != source.image_address_fixup
                {
                    return Err(mismatch);
                }
                match (source.branch.as_deref(), row.branch.as_deref()) {
                    (None, None)
                        if row.bytes == source.bytes && row.alternative == source.alternative => {}
                    (Some(previous), Some(current)) => {
                        let options = forms.get(row.instruction).ok_or(mismatch.clone())?;
                        let form = options
                            .matching(row.alternative, row.bytes.len())
                            .ok_or(mismatch.clone())?;
                        let unchanged_form = row.alternative == source.alternative
                            && row.bytes.len() == source.bytes.len();
                        let shortened_conditional = matches!(
                            current,
                            machine_code::ResolvedBranchEvidence::Conditional(_)
                        ) && options
                            .shortened
                            .as_ref()
                            .is_some_and(|short| std::ptr::eq(short, form))
                            && row.bytes.len() < source.bytes.len();
                        if !(unchanged_form || shortened_conditional)
                            || !branch_preserves_edge(previous, current, form, row, end, &offsets)
                        {
                            return Err(mismatch);
                        }
                    }
                    _ => return Err(mismatch),
                }
                offset = end;
            }
            if block.byte_count != offset - block.offset {
                return Err(mismatch);
            }
        }
        if after.byte_count != offset {
            return Err(mismatch);
        }
    }
    Ok(())
}

fn branch_preserves_edge(
    original: &machine_code::ResolvedBranchEvidence,
    current: &machine_code::ResolvedBranchEvidence,
    form: &machine_code::EncodedBranchAlternative,
    row: &machine_code::ResolvedSelectedFormRow,
    end: u64,
    offsets: &std::collections::BTreeMap<selected_instructions::SelectedBlockId, u64>,
) -> bool {
    use machine_code::ResolvedBranchEvidence::{Conditional, Jump};
    let (target_block, target_offset, displacement) = match (original, current) {
        (Jump(before), Jump(after))
            if before.source_block == after.source_block
                && before.target_edge == after.target_edge
                && before.target_block == after.target_block
                && before.decoded_effects == after.decoded_effects
                && after.decoded_effects == form.footprint.encoded =>
        {
            (
                after.target_block,
                after.target_offset,
                after.byte_displacement,
            )
        }
        (Conditional(before), Conditional(after))
            if before.predicate == after.predicate
                && before.source_block == after.source_block
                && before.when_taken_edge == after.when_taken_edge
                && before.when_taken_block == after.when_taken_block
                && before.when_fallthrough_edge == after.when_fallthrough_edge
                && before.when_fallthrough_block == after.when_fallthrough_block
                && offsets.get(&after.when_fallthrough_block) == Some(&end)
                && after.when_fallthrough_offset == end
                && before.decoded_effects == after.decoded_effects
                && after.decoded_effects == form.footprint.encoded
                && before.decoded_register_reads == after.decoded_register_reads
                && after.decoded_register_reads == form.footprint.register_reads =>
        {
            (
                after.when_taken_block,
                after.when_taken_offset,
                after.byte_displacement,
            )
        }
        _ => return false,
    };
    offsets.get(&target_block) == Some(&target_offset)
        && i128::from(displacement) == i128::from(target_offset) - i128::from(end)
        && form.read_target(&row.bytes, row.offset, target::Architecture::X86_64)
            == Some(target_offset)
}
