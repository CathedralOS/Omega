//! Place the encoded instructions and resolve their function-relative offsets.
#![forbid(unsafe_code)]
pub use encoding_admission::layout::OptimizedResolvedSelectedFormLayoutError;
use encoding_admission::layout::geometry::declared_size_matches;
use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedJumpEvidence,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    SelectedFunctionLayoutPolicy,
};
use machine_code::{
    ResolvedMachineLayout, SelectedFormEncoding,
    resolved_machine_layout_identity as layout_identity,
};
use std::collections::BTreeMap;
use std::sync::Arc;
#[cfg(test)]
mod order_suite;
mod ordinary;
#[cfg(test)]
#[path = "validation_tests.rs"]
mod validation;

/// Layout issues the resolved program and the alternatives available for each control row.
pub struct PlacedProgram<Contracts> {
    program: machine_code::ResolvedMachineProgram,
    contracts: machine_code::layout::EncodedBodyContracts<Contracts>,
}

impl<Contracts> PlacedProgram<Contracts> {
    pub fn program(&self) -> &machine_code::ResolvedMachineProgram {
        &self.program
    }
    pub fn into_parts(
        self,
    ) -> (
        machine_code::ResolvedMachineProgram,
        machine_code::layout::EncodedBodyContracts<Contracts>,
    ) {
        (self.program, self.contracts)
    }
}

/// Only the preceding stage can issue the input for this transformation.
///
/// ```compile_fail,E0308
/// fn skip_stage(raw: machine_code::EncodedMachineProgram) {
///     let _ = selected_form_encoding_to_resolved_layout::place_encoded_program(raw);
/// }
/// ```
pub fn place_encoded_program<Contracts>(
    issued: native_encoding::StagedOptimizedSelectedFormEncoding<Contracts>,
) -> Result<PlacedProgram<Contracts>, OptimizedResolvedSelectedFormLayoutError> {
    let (encoding, contracts) = issued.into_parts();
    let layout = compute(
        &encoding.layout_requirements,
        encoding.context.physical(),
        &encoding.encoding,
    )?;
    let control_alternatives =
        machine_code::EncodedControlIndex::for_layout(&encoding.encoding, &layout)
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::RootMismatch)?;
    let program = machine_code::ResolvedMachineProgram {
        control_alternatives: control_alternatives.into(),
        bindings: encoding.bindings,
        exit_requirements: encoding.exit_requirements,
        fragment_origins: encoding.fragment_origins,
        layout_requirements: encoding.layout_requirements,
        frame_demands: encoding.frame_demands,
        storage: encoding.storage,
        frame_identity: encoding.frame_identity,
        protocol_identity: encoding.protocol_identity,
        context: encoding.context,
        encoding: encoding.encoding,
        layout: Arc::new(layout),
        frame: encoding.frame,
        protocol: encoding.protocol,
    };
    if contracts.audit_requested() {
        encoding_admission::layout::validate_baseline_resolved_layout(&program)?;
    }
    Ok(PlacedProgram { contracts, program })
}
fn compute(
    input: &machine_code::layout_requirements::EncodedLayoutRequirements,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
) -> Result<ResolvedMachineLayout, OptimizedResolvedSelectedFormLayoutError> {
    let selected_plan = input;
    let machine = input;
    let machine_plan = machine;
    if pre_layout.selected() != machine.selected
        || pre_layout.machine() != machine.machine
        || machine.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.alternatives.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let policy = ordinary::select(&selected_plan.functions)?;
    let mut pre_rows = pre_layout.rows().iter();
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for (function, machine_function) in selected_plan
        .functions
        .iter()
        .zip(&machine_plan.alternatives)
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
    let machine_root = machine.machine;
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

pub mod entry_layout;
pub mod program_entry_layout;

/// A fixed-width entry with its body-call or relocation fields resolved.
pub struct ResolvedEntry<T>(T);

impl<T> ResolvedEntry<T> {
    pub fn into_layout(self) -> T {
        self.0
    }
}
