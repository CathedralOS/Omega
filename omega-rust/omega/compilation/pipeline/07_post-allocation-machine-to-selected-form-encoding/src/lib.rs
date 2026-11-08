/*
We encode a post-allocation physical machine plan (`PostAllocationMachinePlan`)
into a layout-independent selected-form encoding (`SelectedFormEncoding`),
translating each chosen machine alternative and physical operand footprint into
target instruction bytes, decoded footprints, and unresolved fixup templates
before block and instruction addresses are assigned.

Both our producer (`stage_optimized_layout_independent_selected_form_encoding`)
and our independent replay (`validate_optimized_layout_independent_selected_form_encoding`)
walk the same lockstep correspondence (`for_each_row`, `:RowCorrespondence`)
between `CurrentSelectedProgram` and `PostAllocationMachinePlan`:

    (SelectedFunction, SelectedInstruction, PostAllocationMachineFunction, PostAllocationMachineInstruction)
      |
      +-- 1. frame_address::resolve (:FrameAddressEquations)
      |        Resolves symbolic local, outgoing, incoming, and hosted frame slots
      |        into signed post-prologue stack displacements against TargetFrameLayoutPlan.
      |
      +-- 2. row_encoding::encode_row (:RouteDrivenEncoding)
      |        Routes each instruction kind to its target ISA encoder (x86-64 or AArch64):
      |        - ResolvedAddress -> memory, frame, or hosted byte-channel bytes
      |        - InternalCallTemplate -> scalar call bytes + UnresolvedInternalMachineCall fixup
      |        - NormalizedForeignCallTemplate -> foreign/table/syscall bytes + import/callback fixups
      |        - ByteLiteralAddress -> PC-relative address bytes + UnresolvedImageAddress fixup
      |        - DeferredControlFlow -> DeferredControl (waits for Stage 08 branch layout)
      |        - Ordinary -> self-contained scalar or hosted-exit bytes
      |
      +-- 3. Intern bytes + SelectedFormDecodedFootprint into SelectedFormEncodedRowStore
             and classify row states into SelectedFormEncodingCounts.

Control-flow instructions (`Jump` and conditional branches) record
`DeferredControl` rows without bytes here because their encodings depend on the
function-relative block offsets computed in Stage 08.
*/

/*
Why we separate layout-independent encoding from branch layout and replay:

Most instructions in a function have fixed byte encodings once their physical
registers and stack-frame displacements are known. Encoding them in Stage 07
isolates target instruction encoding, operand-footprint verification, call
template formation, and frame-offset arithmetic from the control-flow layout and
branch-relaxation passes in Stages 08 and 09.

To prevent a routing or encoding defect from self-certifying, our independent
replay (`validation::validate`, `:IndependentByteReplay`) never calls the
producer's route table or encoders. Instead, it checks the frame-space address
equations directly (`frame_address::validate_address`) and invokes the target
ISA's independent byte decoders (`isa_x86_64` / `isa_aarch64`) to verify that
every interned byte slice decodes to the declared alternative, register views,
implicit unit effects, and size bounds.
*/

/*
Provenance and custody boundaries:

We consume `CurrentSelectedProgram` (`selected-instructions`),
`PostAllocationMachinePlan` (`physical-instructions`, produced by Stage 06),
`ValidatedPhysicalRegisterModel` (`register-model`), and an optional
`TargetFrameLayoutPlan` (`machine-code`). We produce
`StagedOptimizedSelectedFormEncoding`, which wraps an immutable
`SelectedFormEncoding` (`machine-code`) bound by its canonical content digest
and row-state counts. Stage 08 (`selected-form-encoding-to-resolved-layout`)
consumes this encoding to assign dense function-relative byte offsets and
encode deferred branch instructions.
*/

#![forbid(unsafe_code)]

mod frame_address;
mod row_encoding;
mod validation;

use std::sync::Arc;

use isa_aarch64::{
    Aarch64NormalizedForeignCallTemplateError, Aarch64ScalarCallTemplateError,
    Aarch64SelectedFormEncodingError,
};
use isa_x86_64::{
    X86_64MovR32Imm32I64MaterializationError, X86_64MovR64Imm32SignExtendedI64MaterializationError,
    X86_64NormalizedForeignCallTemplateError, X86_64ScalarCallTemplateError,
    X86_64SelectedFormEncodingError,
};
use machine_code::{
    SelectedFormEncodedRowStore, SelectedFormEncoding, SelectedFormEncodingCounts,
    SelectedFormEncodingIdentity, SelectedFormEncodingRow, SelectedFormEncodingState,
    TargetFrameLayoutPlan,
};
use physical_instructions::{
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachineOptimizationCustody, PostAllocationMachinePlan,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    CurrentSelectedProgram, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionPlanIdentity, terminator_instruction,
};

pub fn stage_optimized_layout_independent_selected_form_encoding(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&TargetFrameLayoutPlan>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute(selected, machine, physical, frame)?;
    Ok(StagedOptimizedSelectedFormEncoding {
        program: Arc::new(artifact),
    })
}

pub fn validate_optimized_layout_independent_selected_form_encoding(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&TargetFrameLayoutPlan>,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validation::validate(selected, machine, physical, frame, artifact.program())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding {
    program: Arc<SelectedFormEncoding>,
}

impl StagedOptimizedSelectedFormEncoding {
    pub fn program(&self) -> &SelectedFormEncoding {
        &self.program
    }

    pub fn shared_program(&self) -> Arc<SelectedFormEncoding> {
        Arc::clone(&self.program)
    }

    pub fn selected(&self) -> SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> PostAllocationMachineIdentity {
        self.program.machine
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.program.post_allocation_machine_optimization
    }

    pub fn identity(&self) -> SelectedFormEncodingIdentity {
        self.program.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.program.rows
    }

    pub fn encoded(&self) -> &SelectedFormEncodedRowStore {
        self.program.encoded()
    }

    pub fn counts(&self) -> SelectedFormEncodingCounts {
        self.program.counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedSelectedFormEncodingError {
    SelectedRootMismatch,
    PhysicalModelMismatch,
    FunctionRosterMismatch,
    BlockRosterMismatch,
    InstructionRosterMismatch,
    OperandFootprintMismatch(SelectedInstructionId),
    ImplicitFootprintMismatch(SelectedInstructionId),
    SizeDeclarationMismatch(SelectedInstructionId),
    CountOverflow,
    X86_64(X86_64SelectedFormEncodingError),
    X86_64ScalarCall(X86_64ScalarCallTemplateError),
    X86_64NormalizedForeignCall(X86_64NormalizedForeignCallTemplateError),
    X86_64MovR32Imm32(X86_64MovR32Imm32I64MaterializationError),
    X86_64MovR64Imm32SignExtended(X86_64MovR64Imm32SignExtendedI64MaterializationError),
    Aarch64(Aarch64SelectedFormEncodingError),
    Aarch64ScalarCall(Aarch64ScalarCallTemplateError),
    Aarch64NormalizedForeignCall(Aarch64NormalizedForeignCallTemplateError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedSelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized selected-form encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectedFormEncodingError {}

fn compute(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&TargetFrameLayoutPlan>,
) -> Result<SelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    frame_address::validate_frame_root(machine, frame)?;
    if machine.selected != selected.identity {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let target = selected.plan.target;
    let row_count = machine.instructions().count();
    let mut rows = Vec::with_capacity(row_count);
    let mut encoded = SelectedFormEncodedRowStore::with_row_capacity(row_count);
    for_each_row(
        selected,
        machine,
        |selected_function, selected_instruction, machine_function, machine_instruction| {
            rows.push(row_encoding::encode_row(
                target,
                selected_instruction,
                machine_instruction,
                physical,
                selected_function,
                frame_address::resolve(machine_function, frame, machine_instruction)?,
                &mut encoded,
            )?);
            Ok(())
        },
    )?;
    let counts = counts_of(&rows)?;
    let mut program = SelectedFormEncoding {
        selected: selected.identity,
        machine: machine.identity,
        post_allocation_machine_optimization: None,
        identity: SelectedFormEncodingIdentity::from_bytes([0; 32]),
        rows,
        encoded,
        frame: frame.cloned(),
        counts,
    };
    program.identity = program.recomputed_identity();
    Ok(program)
}

pub(crate) fn for_each_row<V>(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    mut visit: V,
) -> Result<(), OptimizedSelectedFormEncodingError>
where
    V: FnMut(
        &SelectedFunction,
        &SelectedInstruction,
        &PostAllocationMachineFunction,
        &PostAllocationMachineInstruction,
    ) -> Result<(), OptimizedSelectedFormEncodingError>,
{
    let selected_plan = selected.plan;
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    for (selected_function, machine_function) in
        selected_plan.functions.iter().zip(&machine.functions)
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        for (selected_block, machine_block) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let selected_instruction = if index < selected_block.instructions.len() {
                    &selected_block.instructions[index]
                } else {
                    terminator_instruction(&selected_block.terminator)
                };
                if selected_instruction.id != machine_instruction.instruction {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                visit(
                    selected_function,
                    selected_instruction,
                    machine_function,
                    machine_instruction,
                )?;
            }
        }
    }
    Ok(())
}

pub(crate) fn counts_of(
    rows: &[SelectedFormEncodingRow],
) -> Result<SelectedFormEncodingCounts, OptimizedSelectedFormEncodingError> {
    let mut counts = SelectedFormEncodingCounts::default();
    for row in rows {
        let count = match row.state {
            SelectedFormEncodingState::Encoded => &mut counts.ordinary_encoded,
            SelectedFormEncodingState::DeferredControl { .. } => {
                &mut counts.ordinary_deferred_control
            }
            SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                counts.ordinary_deferred_internal_control = counts
                    .ordinary_deferred_internal_control
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                &mut counts.ordinary_internal_fixups
            }
            SelectedFormEncodingState::UnresolvedNormalizedForeignCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                &mut counts.ordinary_normalized_foreign_import_fixups
            }
            SelectedFormEncodingState::UnresolvedImageAddress { .. } => {
                &mut counts.ordinary_image_address_fixups
            }
        };
        *count = count
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
    }
    Ok(counts)
}
