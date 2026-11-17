/*
We lower pre-layout selected-form encoding (`SelectedFormEncoding`) into a function-relative resolved machine layout (`StagedOptimizedResolvedSelectedFormLayout`) for Omega stage 08 (`selected-form-encoding-to-resolved-layout`). The producer derives canonical fallthrough order, exact byte offsets, branch encodings and bounded fixup placeholders. AArch64 conditional branches beyond the signed 19-bit word reach widen to an inverted skip plus an unconditional branch. The result retains those computed rows without replaying its own output; explicit validation and retained-artifact readers use `validate_optimized_resolved_selected_form_layout` separately.
*/

/*
Branch displacements depend on block adjacency, and widening a branch shifts later offsets. The producer therefore owns layout as one operation before optional x86-64 relaxation in stage 09. A separate validator re-derives block order, widening, byte spans and content identity when requested; it is not part of ordinary stage construction.
*/

/*
- Entrypoints:
  - `stage_optimized_resolved_selected_form_layout`: computes and retains function-relative block and instruction layout.
  - `validate_optimized_resolved_selected_form_layout`: replays root custody, layout policy selection, canonical block ordering, monotone branch widening, target branch decoding, fixup bounds, and layout identity.
- Custody:
  - `StagedOptimizedResolvedSelectedFormLayout`: holds the producer's immutable `Arc<ResolvedMachineLayout>` and exposes upstream identities, layout policy, and per-function resolved rows.
  - `OptimizedResolvedSelectedFormLayoutError`: enumerates root mismatches, unsupported function shapes, missing or duplicate instructions, branch fallthrough or size mismatches, and target encoding failures.
- Support modules:
  - `ordinary`: selects the function layout policy, orders blocks along mandatory conditional fallthrough chains, converges AArch64 branch widening, and resolves per-row bytes and branch evidence.
  - `validation`: independently reconstructs block order, byte spans, and decoded branch footprints to admit candidate layouts.
*/

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use isa_aarch64::Aarch64SelectedFormEncodingError;
use isa_x86_64::X86_64SelectedFormEncodingError;
pub use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity,
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
};
use machine_code::{
    ResolvedMachineLayout, SelectedFormEncoding, SelectedFormEncodingIdentity,
    resolved_machine_layout_identity as layout_identity,
};
use post_allocation_machine::PostAllocationMachineOptimizationCustody;
use selected_instructions::{MachineSizeKnowledge, SelectedInstructionId};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

#[cfg(test)]
mod order_suite;
mod ordinary;
mod validation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedResolvedSelectedFormLayoutError {
    CurrentProgram(post_allocation_machine::program_validation::PhysicalProgramValidationError),
    RootMismatch,
    MixedOrdinaryAndStructuralFunctions,
    StructuralReturnRosterMismatch(SelectedInstructionId),
    StructuralEncodingMismatch(SelectedInstructionId),
    UnsupportedFunctionShape(MachineId),
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    AlternativeMismatch(SelectedInstructionId),
    UnexpectedEncodingState(SelectedInstructionId),
    OffsetOverflow,
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    BranchSizeMismatch(SelectedInstructionId),
    OptimizationCustodyMismatch,
    OptimizationByteSavingsMismatch,
    X86_64(X86_64SelectedFormEncodingError),
    Aarch64(Aarch64SelectedFormEncodingError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedResolvedSelectedFormLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized resolved selected-form layout failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedResolvedSelectedFormLayoutError {}

pub fn stage_optimized_resolved_selected_form_layout(
    input: &machine_code::EncodedMachineProgram,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let program = Arc::new(compute(&input.instructions, &input.encoding)?);
    Ok(StagedOptimizedResolvedSelectedFormLayout {
        current: machine_code::ResolvedMachineProgram {
            storage: Arc::clone(&input.storage),
            frame_identity: input.frame_identity,
            protocol_identity: input.protocol_identity,
            instructions: input.instructions.clone(),
            encoding: Arc::clone(&input.encoding),
            layout: Arc::clone(&program),
            frame: Arc::clone(&input.frame),
            protocol: Arc::clone(&input.protocol),
        },
        program,
    })
}

/// Explicit replay relates the current layout to its admitted original owners.
pub struct ResolvedLayoutValidation<'source> {
    pub replay: post_allocation_machine::program_validation::PhysicalProgramReplay<'source>,
    pub pre_layout: &'source SelectedFormEncoding,
    pub artifact: &'source StagedOptimizedResolvedSelectedFormLayout,
}

pub fn validate_optimized_resolved_selected_form_layout(
    input: ResolvedLayoutValidation<'_>,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let ResolvedLayoutValidation {
        replay,
        pre_layout,
        artifact,
    } = input;
    post_allocation_machine::program_validation::validate_current_program(
        &artifact.current.instructions,
        replay,
    )
    .map_err(OptimizedResolvedSelectedFormLayoutError::CurrentProgram)?;
    if pre_layout != artifact.current.encoding.as_ref() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    validation::validate(
        &artifact.current.instructions,
        pre_layout,
        artifact.program(),
        false,
    )
}

/// Validate the current layout, including short x86 conditional branches only
/// when the retained phase selection permits that representation.
pub fn validate_current_resolved_layout(
    program: &machine_code::ResolvedMachineProgram,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    post_allocation_machine::program_validation::validate_current_program_integrity(
        &program.instructions,
    )
    .map_err(OptimizedResolvedSelectedFormLayoutError::CurrentProgram)?;
    let allow_x86_short = program
        .instructions
        .publication
        .selections()
        .contains(optimization_core::Optimization::X86RelaxConditionalBranchesToRel8V1);
    if allow_x86_short && program.instructions.target.architecture != target::Architecture::X86_64 {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    validation::validate(
        &program.instructions,
        &program.encoding,
        &program.layout,
        allow_x86_short,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    current: machine_code::ResolvedMachineProgram,
    program: Arc<ResolvedMachineLayout>, // Producer-owned function-relative machine layout.
}

impl StagedOptimizedResolvedSelectedFormLayout {
    pub fn current(&self) -> &machine_code::ResolvedMachineProgram {
        &self.current
    }
    pub fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> post_allocation_machine::PostAllocationMachineIdentity {
        self.program.machine
    }

    pub fn pre_layout(&self) -> SelectedFormEncodingIdentity {
        self.program.pre_layout
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.program.post_allocation_machine_optimization
    }

    pub fn target(&self) -> NativeTarget {
        self.program.target
    }

    pub fn policy(&self) -> SelectedFunctionLayoutPolicy {
        self.program.policy
    }

    pub fn identity(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.program.identity
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        &self.program.functions
    }

    pub fn program(&self) -> &ResolvedMachineLayout {
        &self.program
    }

    pub fn shared_program(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.program)
    }
}

fn compute(
    input: &post_allocation_machine::PostAllocationMachineProgram,
    pre_layout: &SelectedFormEncoding,
) -> Result<ResolvedMachineLayout, OptimizedResolvedSelectedFormLayoutError> {
    let selected_plan = input;
    let machine = &input.machine;
    let machine_plan = machine;
    let physical = input.physical();
    if pre_layout.selected() != machine.selected
        || pre_layout.machine() != machine.identity
        || machine.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let policy = ordinary::select(&selected_plan.functions)?;
    let mut pre_rows = pre_layout.rows().iter();
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for (function, machine_function) in selected_plan.functions.iter().zip(&machine_plan.functions)
    {
        let mut function_pre_rows = BTreeMap::new();
        for block in &function.blocks {
            for instruction in ordinary::instructions(block) {
                let row = pre_rows.next().ok_or(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
                )?;
                if row.instruction != instruction.id {
                    return Err(
                        OptimizedResolvedSelectedFormLayoutError::MissingInstruction(
                            instruction.id,
                        ),
                    );
                }
                if function_pre_rows.insert(instruction.id, row).is_some() {
                    return Err(
                        OptimizedResolvedSelectedFormLayoutError::DuplicateInstruction(
                            instruction.id,
                        ),
                    );
                }
            }
        }
        let machine_rows = machine_function
            .instructions()
            .map(|instruction| (instruction.instruction, instruction))
            .collect::<BTreeMap<_, _>>();
        functions.push(ordinary::layout(
            machine.target.architecture,
            function,
            &function_pre_rows,
            &machine_rows,
            physical,
            policy,
            pre_layout.encoded(),
        )?);
    }
    if pre_rows.next().is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let selected_root = machine.selected;
    let machine_root = machine.identity;
    let pre_layout_root = pre_layout.identity();
    let target = machine.target;
    let identity = layout_identity(
        selected_root,
        machine_root,
        pre_layout_root,
        None,
        target,
        policy,
        &functions,
    );
    Ok(ResolvedMachineLayout {
        selected: selected_root,
        machine: machine_root,
        pre_layout: pre_layout_root,
        post_allocation_machine_optimization: None,
        target,
        policy,
        identity,
        functions,
    })
}

pub(crate) fn declared_size_matches(knowledge: MachineSizeKnowledge, actual: u64) -> bool {
    match knowledge {
        MachineSizeKnowledge::ExactBytes(expected) => u64::from(expected) == actual,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            actual >= u64::from(minimum_bytes)
                && maximum_bytes.is_none_or(|maximum| actual <= u64::from(maximum))
        }
    }
}

pub(crate) fn is_admitted_canonical_shape(
    function: &post_allocation_machine::PhysicalFunction,
) -> bool {
    !function.blocks.is_empty() && function.block(function.entry_block).is_some()
}

pub(crate) fn is_single_entry(function: &post_allocation_machine::PhysicalFunction) -> bool {
    let Some(block) = function
        .blocks
        .first()
        .filter(|_| function.blocks.len() == 1)
    else {
        return false;
    };
    function.entry_block == block.id
        && matches!(
            block.terminator,
            selected_instructions::SelectedTerminator::Return { .. }
        )
}
