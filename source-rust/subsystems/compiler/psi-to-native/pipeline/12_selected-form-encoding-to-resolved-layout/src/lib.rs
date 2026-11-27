//! Resolve the function-relative offsets of encoded instructions, including selected branch rewrites.
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

pub mod relaxation;
mod settled;
mod x86_branch_relaxation;

pub use settled::{
    FunctionExitLayout, ResolvedFunctionLayout, ResolvedLayoutOptimizationError,
    SettledFunctionLayout,
};
pub use x86_branch_relaxation::StagedOptimizedX86BranchRelaxation;

#[derive(Debug)]
pub enum LayoutError {
    Baseline(OptimizedResolvedSelectedFormLayoutError),
    Relaxation(ResolvedLayoutOptimizationError),
}

impl LayoutError {
    /// The selected optimization that this target has no implementation of, when that is
    /// why layout failed.
    pub fn unimplemented_optimization(&self) -> Option<optimization_core::Optimization> {
        use machine_code::layout::relaxation_policy::FunctionRelativeLayoutCatalogError;
        match self {
            Self::Relaxation(ResolvedLayoutOptimizationError::Catalog(
                FunctionRelativeLayoutCatalogError::UnsupportedTarget { optimization, .. },
            )) => Some(*optimization),
            _ => None,
        }
    }
}

/// The issued function layout and the optional input to independent replay.
pub struct LayoutResolution<Contracts> {
    pub layout: ResolvedFunctionLayout<Contracts>,
    pub baseline_for_audit: Option<machine_code::ResolvedMachineProgram>,
}

/// Resolve encoded rows to their final offsets under the selected branch policy.
/// Independent replay also receives the original near-branch layout when requested.
///
/// ```compile_fail,E0308
/// fn skip_encoding(raw: machine_code::EncodedMachineProgram) {
///     let _ = selected_form_encoding_to_resolved_layout::resolve_program(raw);
/// }
/// ```
pub fn resolve_program<Contracts>(
    issued: native_encoding::StagedOptimizedSelectedFormEncoding<Contracts>,
) -> Result<LayoutResolution<Contracts>, LayoutError> {
    let (encoding, contracts) = issued.into_parts();
    let program = baseline(encoding).map_err(LayoutError::Baseline)?;
    let baseline_for_audit = if contracts.audit_requested() {
        encoding_admission::layout::validate_baseline_resolved_layout(&program)
            .map_err(LayoutError::Baseline)?;
        Some(program.clone())
    } else {
        None
    };
    let layout = settled::settle(program, contracts).map_err(LayoutError::Relaxation)?;
    Ok(LayoutResolution {
        layout,
        baseline_for_audit,
    })
}

fn baseline(
    encoding: machine_code::EncodedMachineProgram,
) -> Result<machine_code::ResolvedMachineProgram, OptimizedResolvedSelectedFormLayoutError> {
    let layout = compute(
        &encoding.layout_requirements,
        encoding.context.physical(),
        &encoding.encoding,
    )?;
    let control_alternatives =
        machine_code::EncodedControlIndex::for_layout(&encoding.encoding, &layout)
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::RootMismatch)?;
    Ok(machine_code::ResolvedMachineProgram {
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
    })
}
fn compute(
    input: &machine_code::layout_requirements::EncodedLayoutRequirements,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    pre_layout: &SelectedFormEncoding,
) -> Result<ResolvedMachineLayout, OptimizedResolvedSelectedFormLayoutError> {
    if pre_layout.selected() != input.selected
        || pre_layout.machine() != input.machine
        || input.target.architecture != physical.model().architecture
        || input.functions.len() != input.alternatives.len()
        || pre_layout.post_allocation_machine_optimization().is_some()
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let policy = ordinary::select(&input.functions)?;
    let mut pre_rows = pre_layout.rows().iter();
    let mut functions = Vec::with_capacity(input.functions.len());
    for (function, machine_function) in input.functions.iter().zip(&input.alternatives) {
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
            input.target.architecture,
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

    let selected_root = input.selected;
    let machine_root = input.machine;
    let pre_layout_root = pre_layout.identity();
    let target = input.target;
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
