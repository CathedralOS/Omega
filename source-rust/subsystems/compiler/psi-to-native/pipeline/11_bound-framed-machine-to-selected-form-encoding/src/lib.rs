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
    SelectedFormEncodingRow, TargetFrameLayoutPlan,
};
use post_allocation_machine::PostAllocationMachinePlan;
use selected_instructions::terminator_instruction;
use std::sync::Arc;

/// Only a body with bound installation resources can reach encoding.
/// ```compile_fail,E0308
/// fn skip_binding(body: entry_planning::imports::PreparedFunctionBody) {
///     let _ = native_encoding::encode_bound_function_body(body);
/// }
/// ```
/// Only stage 09's installation binding imports the issuer.
/// ```compile_fail,E0432
/// use installation_binding_issuer::BINDER;
/// ```
/// The encoder cannot issue a body from its own recipes and a set of requirements.
/// ```compile_fail,E0308
/// fn forge<F, I, A, P>(
///     plan: physical_recipes::frame_protocol::PlannedFunctionBody<F, I>,
///     installations: std::sync::Arc<A>, requirements: machine_code::layout::BoundEncodingRequirements<P>,
/// ) {
///     let _ = bound_framed_machine::BoundFunctionBody::from_binding((), plan, installations, requirements);
/// }
/// ```
/// An independent flag cannot change which originals this body retains.
///
/// ```compile_fail,E0061
/// fn change_request<F, I, A, P>(body: bound_framed_machine::BoundFunctionBody<F, I, A, P>) {
///     let _ = native_encoding::encode_bound_function_body(body, true);
/// }
/// ```
///
/// The encoder can carry any publication issued by binding. Its implementation
/// cannot ask for an earlier target plan.
/// ```compile_fail,E0616
/// fn target_graph<F, I, A, P>(body: bound_framed_machine::BoundFunctionBody<F, I, A, P>) {
///     let (_, binding) = body.into_encoding_parts();
///     let _ = binding.publication.original_for_audit();
/// }
/// ```
pub fn encode_bound_function_body<FrameOrigin, ImportOrigin, Installations, Publication>(
    body: bound_framed_machine::BoundFunctionBody<
        FrameOrigin,
        ImportOrigin,
        Installations,
        Publication,
    >,
) -> Result<
    StagedOptimizedSelectedFormEncoding<machine_code::layout::EncodedBodyPublication<Publication>>,
    FunctionBodyEncodingError,
> {
    let (program, requirements) = body.into_encoding_parts();
    let original = requirements.capture_original(&program.frame.instructions);
    let audit = original.is_some();
    let input = frame_protocol::encode_function_frame(program, audit)?;
    let current = encode_program(input, audit).map_err(FunctionBodyEncodingError::Encoding)?;
    let contracts = requirements
        .finish(&current, original)
        .map_err(FunctionBodyEncodingError::BodyContracts)?;
    Ok(StagedOptimizedSelectedFormEncoding { current, contracts })
}

/// Encode every row and publish the encoded program.
///
/// `compute` enforces each row's encoding contracts as it encodes: byte length
/// against the declared size, encoded effects against the allocator's
/// alternative, external operands, and the frame-address equation. Replaying
/// the program through encoding admission re-encodes every row and re-hashes
/// the artifact to compare it with itself, so only an audited body (`audit`,
/// set when the body retains its originals) pays for that replay.
fn encode_program(
    input: machine_code::FramedMachineProgram,
    audit: bool,
) -> Result<machine_code::EncodedMachineProgram, OptimizedSelectedFormEncodingError> {
    let artifact = compute(&input.instructions, &input.frame)?;
    let program = Arc::new(artifact);
    if audit {
        validate_current_selected_form_encoding(&input.instructions, &input.frame, &program)?;
    }
    let context = Arc::new(program_context(
        &input.instructions,
        Arc::clone(&input.frame_demands),
        machine_code::SelectedFormEncodingHandle::of(&program),
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

    pub fn machine(&self) -> &Arc<PostAllocationMachinePlan> {
        &self.current.encoding.machine
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
    shared_frame: &Arc<TargetFrameLayoutPlan>,
) -> Result<SelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let frame = Some(&**shared_frame);
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
                let address = frame_address::resolve(machine_function, frame, machine_instruction)?;
                // The resolved address must satisfy the frame-space equation
                // admission states; this is cheap and runs on every build.
                frame_address::validate_address(
                    machine_function,
                    frame,
                    machine_instruction,
                    address,
                )?;
                rows.push(row_encoding::encode_row(
                    target,
                    operation,
                    machine_instruction,
                    physical,
                    &function.normalized_foreign_calls,
                    address,
                    &mut encoded,
                )?);
            }
        }
    }
    let counts = counts_of(&rows)?;
    Ok(SelectedFormEncoding {
        machine: Arc::clone(machine),
        rows,
        encoded: Arc::new(encoded),
        frame: Some(Arc::clone(shared_frame)),
        counts,
    })
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
