//! Encode the complete framed physical program before layout.
//!
//! This producer owns row routing and instruction templates. Encoding admission
//! owns independent byte checks and shared current-form contract projections.

#![forbid(unsafe_code)]
mod control_encoding;
mod row_encoding;
pub use encoding_admission::OptimizedSelectedFormEncodingError;
use encoding_admission::{
    counts_of, exit_requirements, frame_address, layout_requirements, program_context,
    validate_current_selected_form_encoding,
};
use machine_code::{
    SelectedFormEncodedRowStore, SelectedFormEncoding, SelectedFormEncodingCounts,
    SelectedFormEncodingIdentity, SelectedFormEncodingRow, TargetFrameLayoutPlan,
};
use post_allocation_machine::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};
use selected_instructions::{SelectedInstructionPlanIdentity, terminator_instruction};
use std::sync::Arc;

pub fn stage_optimized_layout_independent_selected_form_encoding(
    input: machine_code::FramedMachineProgram,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute(&input.instructions, Some(&input.frame))?;
    let program = Arc::new(artifact);
    validate_current_selected_form_encoding(&input.instructions, &input.frame, &program)?;
    let context = Arc::new(program_context(
        &input.instructions,
        Arc::clone(&input.frame_demands),
        program.identity(),
    ));
    Ok(StagedOptimizedSelectedFormEncoding {
        current: machine_code::EncodedMachineProgram {
            exit_requirements: Arc::new(
                exit_requirements::prepare_exit_requirements(&input.instructions, &program)
                    .map_err(OptimizedSelectedFormEncodingError::ExitRequirements)?,
            ),
            fragment_origins: input.fragment_origins,
            layout_requirements: Arc::new(layout_requirements::prepare_layout_requirements(
                &input.instructions,
            )),
            frame_demands: input.frame_demands,
            storage: input.storage,
            frame_identity: input.frame_identity,
            protocol_identity: input.protocol_identity,
            context,
            frame: input.frame,
            protocol: input.protocol,
            encoding: Arc::clone(&program),
        },
        program,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding {
    current: machine_code::EncodedMachineProgram,
    program: Arc<SelectedFormEncoding>,
}

impl StagedOptimizedSelectedFormEncoding {
    pub fn into_current(self) -> machine_code::EncodedMachineProgram {
        self.current
    }

    pub fn current(&self) -> &machine_code::EncodedMachineProgram {
        &self.current
    }
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

fn compute(
    input: &post_allocation_machine::PostAllocationMachineProgram,
    frame: Option<&TargetFrameLayoutPlan>,
) -> Result<SelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let machine = &input.machine;
    let physical = input.physical();
    frame_address::validate_frame_root(machine, frame)?;
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let target = machine.target;
    let row_count = machine.instructions().count();
    let mut rows = Vec::with_capacity(row_count);
    let mut encoded = SelectedFormEncodedRowStore::with_row_capacity(row_count);
    if input.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    for (function, machine_function) in input.functions.iter().zip(&machine.functions) {
        if function.machine != machine_function.machine
            || function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        for (block, machine_block) in function.blocks.iter().zip(&machine_function.blocks) {
            if block.id != machine_block.block
                || block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            for (operation, machine_instruction) in block
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(&block.terminator)))
                .zip(&machine_block.instructions)
            {
                if operation.id != machine_instruction.instruction {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                rows.push(row_encoding::encode_row(
                    target,
                    operation,
                    machine_instruction,
                    physical,
                    &function.normalized_foreign_calls,
                    frame_address::resolve(machine_function, frame, machine_instruction)?,
                    &mut encoded,
                )?);
            }
        }
    }
    let counts = counts_of(&rows)?;
    let mut program = SelectedFormEncoding {
        selected: machine.selected,
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

mod frame_protocol;
pub use frame_protocol::encode_function_frame;

pub mod adapters;
pub mod callable_entry;
pub mod hosted_entry;
pub mod hosted_program;
pub mod imports;
pub mod native_entry_bridge;
pub mod native_foreign_bridge;
pub mod physical_entry;
pub mod physical_stack_switch;
pub mod windows_internet;
