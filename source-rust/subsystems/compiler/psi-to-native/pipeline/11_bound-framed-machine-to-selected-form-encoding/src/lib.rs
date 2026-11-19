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

/// Only a body with bound installation resources can reach encoding.
/// ```compile_fail,E0308
/// fn skip_binding(body: entry_planning::imports::PreparedFunctionBody) {
///     let _ = native_encoding::encode_bound_function_body(body);
/// }
/// ```
/// An independent flag cannot change which originals this body retains.
///
/// ```compile_fail,E0061
/// fn change_request(body: installation_binding::BoundFunctionBody) {
///     let _ = native_encoding::encode_bound_function_body(body, true);
/// }
/// ```
///
/// The encoder can carry any publication issued by binding. Its implementation
/// cannot ask for an earlier target plan.
/// ```compile_fail,E0616
/// fn target_graph(body: installation_binding::BoundFunctionBody) {
///     let (_, binding) = body.into_encoding_parts();
///     let _ = binding.publication.original_for_audit();
/// }
/// ```
pub fn encode_bound_function_body<Publication>(
    body: installation_binding::BoundFunctionBody<Publication>,
) -> Result<
    StagedOptimizedSelectedFormEncoding<machine_code::layout::EncodedBodyPublication<Publication>>,
    FunctionBodyEncodingError,
> {
    let (program, requirements) = body.into_encoding_parts();
    let original = requirements.capture_original(&program.frame.instructions);
    let input = frame_protocol::encode_function_frame(program, original.is_some())?;
    let current = encode_program(input).map_err(FunctionBodyEncodingError::Encoding)?;
    let contracts = requirements
        .finish(&current, original)
        .map_err(FunctionBodyEncodingError::BodyContracts)?;
    Ok(StagedOptimizedSelectedFormEncoding { current, contracts })
}

fn encode_program(
    input: machine_code::FramedMachineProgram,
) -> Result<machine_code::EncodedMachineProgram, OptimizedSelectedFormEncodingError> {
    let artifact = compute(&input.instructions, Some(&input.frame))?;
    let program = Arc::new(artifact);
    validate_current_selected_form_encoding(&input.instructions, &input.frame, &program)?;
    let context = Arc::new(program_context(
        &input.instructions,
        Arc::clone(&input.frame_demands),
        program.identity(),
    ));
    Ok(machine_code::EncodedMachineProgram {
        bindings: input.bindings,
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
        encoding: program,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding<Contracts> {
    current: machine_code::EncodedMachineProgram,
    contracts: machine_code::layout::EncodedBodyContracts<Contracts>,
}

impl<Contracts> StagedOptimizedSelectedFormEncoding<Contracts> {
    pub fn into_parts(
        self,
    ) -> (
        machine_code::EncodedMachineProgram,
        machine_code::layout::EncodedBodyContracts<Contracts>,
    ) {
        (self.current, self.contracts)
    }

    pub fn current(&self) -> &machine_code::EncodedMachineProgram {
        &self.current
    }
    pub fn program(&self) -> &SelectedFormEncoding {
        &self.current.encoding
    }

    pub fn shared_program(&self) -> Arc<SelectedFormEncoding> {
        Arc::clone(&self.current.encoding)
    }

    pub fn selected(&self) -> SelectedInstructionPlanIdentity {
        self.current.encoding.selected
    }

    pub fn machine(&self) -> PostAllocationMachineIdentity {
        self.current.encoding.machine
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.current.encoding.post_allocation_machine_optimization
    }

    pub fn identity(&self) -> SelectedFormEncodingIdentity {
        self.current.encoding.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.current.encoding.rows
    }

    pub fn encoded(&self) -> &SelectedFormEncodedRowStore {
        self.current.encoding.encoded()
    }

    pub fn counts(&self) -> SelectedFormEncodingCounts {
        self.current.encoding.counts
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
        encoded: Arc::new(encoded),
        frame: frame.cloned(),
        counts,
    };
    program.identity = program.recomputed_identity();
    Ok(program)
}

mod frame_protocol;
pub use frame_protocol::FunctionBodyEncodingError;

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
