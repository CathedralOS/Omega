/*
We lower pre-layout selected-form encoding (`SelectedFormEncoding`) into a function-relative resolved machine layout (`StagedOptimizedResolvedSelectedFormLayout`) for Omega stage 08 (`selected-form-encoding-to-resolved-layout`). Starting from the selected instruction plan, the post-allocation machine plan, the validated physical register model, and the pre-layout encoding rows, `stage_optimized_resolved_selected_form_layout` derives a canonical fallthrough block order for each function, computes exact block and instruction byte offsets, resolves deferred branch and jump encodings against those offsets (widening AArch64 conditional branches that exceed the signed 19-bit word reach into an inverted skip plus an unconditional branch), and verifies that internal-machine call, foreign-call, and image-address fixup placeholders remain canonically bounded inside their row spans. Every staged layout is then replayed through `validate_optimized_resolved_selected_form_layout` before we publish it.
*/

/*
Branch displacements and block-relative offsets cannot be fixed while instructions are encoded in isolation during stage 07, because conditional fallthrough chains determine block adjacency and AArch64 out-of-range conditional branches grow by four bytes when widened, shifting every subsequent block and instruction offset in the function. By separating function-relative layout into its own stage with an independent validator that re-derives block order, monotone branch widening, byte offsets, target-decoded branch footprints, and content identity from scratch, we establish a verified baseline layout before optional x86-64 short-branch relaxation runs in stage 09.
*/

/*
- Entrypoints:
  - `stage_optimized_resolved_selected_form_layout`: computes function-relative block and instruction layout and admits the result through independent validation.
  - `validate_optimized_resolved_selected_form_layout`: replays root custody, layout policy selection, canonical block ordering, monotone branch widening, target branch decoding, fixup bounds, and layout identity.
- Custody:
  - `StagedOptimizedResolvedSelectedFormLayout`: holds the validated `Arc<ResolvedMachineLayout>` and exposes its upstream identities, layout policy, and per-function resolved rows.
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
use physical_instructions::{PostAllocationMachineOptimizationCustody, PostAllocationMachinePlan};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    CurrentSelectedProgram, MachineSizeKnowledge, SelectedFunction, SelectedInstructionId,
    SelectedTerminator,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

#[cfg(test)]
mod order_suite;
mod ordinary;
mod validation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedResolvedSelectedFormLayoutError {
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
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = compute(selected, machine, physical, pre_layout)?;
    admit_resolved_machine_layout(selected, machine, physical, pre_layout, artifact.program)
}

pub fn validate_optimized_resolved_selected_form_layout(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    validation::validate(selected, machine, physical, pre_layout, artifact)
}

pub(crate) fn admit_resolved_machine_layout(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
    program: Arc<ResolvedMachineLayout>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = StagedOptimizedResolvedSelectedFormLayout { program };
    validation::validate(selected, machine, physical, pre_layout, &artifact)?;
    Ok(artifact)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    program: Arc<ResolvedMachineLayout>, // Validated function-relative machine layout artifact.
}

impl StagedOptimizedResolvedSelectedFormLayout {
    pub fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
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

    fn from_program(program: ResolvedMachineLayout) -> Self {
        Self {
            program: Arc::new(program),
        }
    }
}

fn compute(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let selected_plan = selected.plan;
    let machine_plan = machine;
    if pre_layout.selected() != selected.identity
        || pre_layout.machine() != machine.identity
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let policy = ordinary::select(selected_plan)?;
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
            selected_plan.target.architecture,
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

    let selected_root = selected.identity;
    let machine_root = machine.identity;
    let pre_layout_root = pre_layout.identity();
    let target = selected_plan.target;
    let identity = layout_identity(
        selected_root,
        machine_root,
        pre_layout_root,
        None,
        target,
        policy,
        &functions,
    );
    Ok(StagedOptimizedResolvedSelectedFormLayout::from_program(
        ResolvedMachineLayout {
            selected: selected_root,
            machine: machine_root,
            pre_layout: pre_layout_root,
            post_allocation_machine_optimization: None,
            target,
            policy,
            identity,
            functions,
        },
    ))
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

pub(crate) fn is_admitted_canonical_shape(function: &SelectedFunction) -> bool {
    !function.blocks.is_empty() && function.block(function.entry_block).is_some()
}

pub(crate) fn is_single_entry(function: &SelectedFunction) -> bool {
    let Some(block) = function
        .blocks
        .first()
        .filter(|_| function.blocks.len() == 1)
    else {
        return false;
    };
    function.entry_block == block.id
        && matches!(block.terminator, SelectedTerminator::Return { .. })
}
