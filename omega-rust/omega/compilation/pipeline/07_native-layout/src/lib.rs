//! Place every encoded row and apply the selected branch relaxation policy.
//! Layout uses the alternatives supplied by encoding. The optional original
//! encoding and baseline serve explicit replay requests.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

pub use encoding_admission::layout::OptimizedResolvedSelectedFormLayoutError;
use encoding_admission::layout::geometry::declared_size_matches;
pub use encoding_admission::relaxation::ResolvedLayoutOptimizationError;
#[cfg(test)]
use encoding_admission::relaxation::{
    FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
};
use encoding_admission::relaxation::{relaxation_roots, x86_rel8_selected};
use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedJumpEvidence,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    SelectedFunctionLayoutPolicy,
};
use machine_code::{
    ResolvedMachineLayout, SelectedFormEncoding,
    resolved_machine_layout_identity as layout_identity,
};
#[cfg(test)]
use optimization_core::Optimization;
use optimization_core::OptimizationExecutionPhase;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use target::Architecture;

mod x86_branch_relaxation;
pub use x86_branch_relaxation::StagedOptimizedX86BranchRelaxation;

#[cfg(test)]
mod order_suite;
mod ordinary;
#[cfg(test)]
#[path = "validation_tests.rs"]
mod validation;

pub struct LayoutObservation<'a> {
    pub placement: &'a AtomicU64,
    pub relaxation: &'a AtomicU64,
}

pub struct LayoutOriginals {
    pub encoding: machine_code::EncodedMachineProgram,
    pub baseline: machine_code::ResolvedMachineProgram,
}

pub struct LayoutOutput {
    pub current: machine_code::OptimizedMachineLayout,
    pub relaxation: Option<StagedOptimizedX86BranchRelaxation>,
    pub originals: Option<LayoutOriginals>,
}

#[derive(Debug)]
pub enum LayoutError {
    Placement(OptimizedResolvedSelectedFormLayoutError),
    Optimization(ResolvedLayoutOptimizationError),
}

pub fn layout_program(
    encoding: machine_code::EncodedMachineProgram,
    retain_originals: bool,
    observation: LayoutObservation<'_>,
) -> Result<LayoutOutput, LayoutError> {
    observation.placement.fetch_add(1, Ordering::Relaxed);
    let layout = compute(
        &encoding.layout_requirements,
        encoding.context.physical(),
        &encoding.encoding,
    )
    .map_err(LayoutError::Placement)?;
    let original_encoding = retain_originals.then(|| encoding.clone());
    let mut program = machine_code::ResolvedMachineProgram {
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
    observation.relaxation.fetch_add(1, Ordering::Relaxed);
    let selections = program
        .context
        .publication
        .selections()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let budget = program.context.publication.budget();
    let enabled =
        x86_rel8_selected(&selections, program.layout.target().architecture).map_err(|error| {
            LayoutError::Optimization(ResolvedLayoutOptimizationError::Catalog(error))
        })?;
    let relaxation = if enabled {
        Some(
            x86_branch_relaxation::stage_optimized_x86_branch_relaxation(
                program.context.physical(),
                &program.layout,
                &program.encoding,
                budget,
            )
            .map_err(|error| {
                LayoutError::Optimization(ResolvedLayoutOptimizationError::Relaxation(error))
            })?,
        )
    } else {
        None
    };
    let baseline = program.layout.roots();
    let originals = original_encoding.map(|encoding| LayoutOriginals {
        encoding,
        baseline: program.clone(),
    });
    if let Some(issued) = &relaxation {
        program.layout = issued.shared_layout();
    }
    Ok(LayoutOutput {
        current: machine_code::OptimizedMachineLayout {
            baseline,
            program,
            selections,
            budget,
            relaxation: relaxation
                .as_ref()
                .map(|issued| relaxation_roots(issued.record())),
        },
        relaxation,
        originals,
    })
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

#[cfg(test)]
mod tests {
    use optimization_core::OptimizationSelections;

    use super::{
        Architecture, FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
        Optimization, OptimizationExecutionPhase, x86_rel8_selected,
    };

    #[test]
    fn ordered_catalog_covers_every_declared_function_relative_layout_optimization_once() {
        let declared = Optimization::ALL
            .into_iter()
            .filter(|optimization| {
                optimization.execution_phase() == OptimizationExecutionPhase::FunctionRelativeLayout
            })
            .collect::<Vec<_>>();
        assert_eq!(
            declared,
            FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG.map(|entry| entry.optimization())
        );
        let selections = OptimizationSelections::new(
            FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG.map(|entry| entry.optimization()),
        )
        .unwrap();
        let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&phase, Architecture::X86_64), Ok(true));
        assert_eq!(
            x86_rel8_selected(&phase, Architecture::Aarch64),
            Err(FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                optimization: Optimization::X86RelaxConditionalBranchesToRel8V1,
                required: Architecture::X86_64,
                actual: Architecture::Aarch64,
            })
        );
    }

    #[test]
    fn selection_vocabulary_routes_phases_and_rejects_foreign_phase_projections() {
        let empty = OptimizationSelections::new([])
            .unwrap()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&empty, Architecture::X86_64), Ok(false));

        let foreign =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&foreign, Architecture::X86_64), Ok(false));

        let mixed = OptimizationSelections::new([
            Optimization::X86RelaxConditionalBranchesToRel8V1,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
        assert_eq!(x86_rel8_selected(&mixed, Architecture::X86_64), Ok(true));

        let wrong_phase =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap()
                .project_phase(OptimizationExecutionPhase::SelectedLowering);
        assert_eq!(
            x86_rel8_selected(&wrong_phase, Architecture::X86_64),
            Err(FunctionRelativeLayoutCatalogError::WrongPhase(
                optimization_core::OptimizationPhaseMismatch {
                    expected: OptimizationExecutionPhase::FunctionRelativeLayout,
                    actual: OptimizationExecutionPhase::SelectedLowering,
                },
            ))
        );
    }
}
