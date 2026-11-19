//! Relax the resolved layout under the program's selected optimization policy and work budget.
#![forbid(unsafe_code)]
pub use machine_code::layout::relaxation_policy::ResolvedLayoutOptimizationError;
#[cfg(test)]
use machine_code::layout::relaxation_policy::{
    FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
};
use machine_code::layout::relaxation_policy::{relaxation_roots, x86_rel8_selected};
#[cfg(test)]
use optimization_core::Optimization;
use optimization_core::OptimizationExecutionPhase;
#[cfg(test)]
use target::Architecture;

pub mod relaxation;
mod x86_branch_relaxation;
pub use x86_branch_relaxation::StagedOptimizedX86BranchRelaxation;

/// The final layout and the trace issued by the pass that produced it.
#[derive(Debug)]
pub struct RelaxedLayout<Contracts> {
    contracts: machine_code::layout::EncodedBodyContracts<Contracts>,
    layout: SettledFunctionLayout,
}

impl<Contracts> RelaxedLayout<Contracts> {
    /// Independent replay compares this stage's output with the captured encoding.
    pub fn layout_for_audit(&self) -> &machine_code::OptimizedMachineLayout {
        self.layout.current()
    }

    pub fn into_placement(
        self,
    ) -> Option<(
        SettledFunctionLayout,
        machine_code::layout::FramePlacementContracts<Contracts>,
    )> {
        let contracts = self
            .contracts
            .into_placement(&self.layout.current.program)?;
        Some((self.layout, contracts))
    }
}

/// Settled instruction offsets, frame geometry and relaxation evidence.
#[derive(Debug)]
pub struct SettledFunctionLayout {
    current: machine_code::OptimizedMachineLayout,
    relaxation: Option<StagedOptimizedX86BranchRelaxation>,
}

/// The final function offsets and the encoder's exit obligations. This view
/// contains all inputs to the ABI exit check, including the selected frame.
///
/// ```compile_fail,E0609
/// fn archived_program(input: resolved_layout_to_resolved_layout::FunctionExitLayout<'_>) {
///     let _ = input.encoding.rows();
/// }
/// ```
#[non_exhaustive]
pub struct FunctionExitLayout<'a> {
    pub exit_requirements: &'a machine_code::exit_requirements::EncodedExitRequirements,
    pub fragment_origins: &'a machine_code::fragment_origins::FragmentOrigins,
    pub frame_demands: &'a machine_code::frame_demands::MachineFrameDemands,
    pub physical: &'a register_model::ValidatedPhysicalRegisterModel,
    pub layout: &'a machine_code::ResolvedMachineLayout,
    pub frame: &'a machine_code::TargetFrameLayoutPlan,
    pub protocol: &'a machine_code::TargetFrameProtocolEncodingPlan,
    pub frame_identity: machine_code::TargetFrameLayoutIdentity,
    pub protocol_identity: machine_code::TargetFrameProtocolEncodingIdentity,
    pub custody: machine_code::WholeFunctionExitLayoutCustody,
}

impl SettledFunctionLayout {
    pub fn current(&self) -> &machine_code::OptimizedMachineLayout {
        &self.current
    }

    pub fn function_exits(&self) -> FunctionExitLayout<'_> {
        let program = &self.current.program;
        let custody = match self.current.relaxation {
            Some(run) => {
                machine_code::WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                    relaxation: run.identity,
                }
            }
            None => machine_code::WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
        };
        FunctionExitLayout {
            exit_requirements: &program.exit_requirements,
            fragment_origins: &program.fragment_origins,
            frame_demands: &program.frame_demands,
            physical: program.context.physical(),
            layout: &program.layout,
            frame: &program.frame,
            protocol: &program.protocol,
            frame_identity: program.frame_identity,
            protocol_identity: program.protocol_identity,
            custody,
        }
    }

    pub fn trace(&self) -> Option<&machine_code::layout::X86BranchRelaxationRecord> {
        self.relaxation.as_ref().map(|issued| issued.record())
    }
}

/// Only the preceding stage can issue the input for this transformation.
///
/// ```compile_fail,E0308
/// fn skip_stage(raw: machine_code::ResolvedMachineProgram) {
///     let _ = resolved_layout_to_resolved_layout::relax_resolved_layout(raw);
/// }
/// ```
pub fn relax_resolved_layout<Contracts>(
    placed: placement::PlacedProgram<Contracts>,
) -> Result<RelaxedLayout<Contracts>, ResolvedLayoutOptimizationError> {
    let (mut program, contracts) = placed.into_parts();
    let selections = program
        .context
        .publication
        .selections()
        .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let budget = program.context.publication.budget();
    let enabled = x86_rel8_selected(&selections, program.layout.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    let relaxation = if enabled {
        Some(
            x86_branch_relaxation::stage_optimized_x86_branch_relaxation(
                program.context.physical(),
                &program.layout,
                &program.control_alternatives,
                budget,
            )
            .map_err(ResolvedLayoutOptimizationError::Relaxation)?,
        )
    } else {
        None
    };
    let baseline = program.layout.roots();
    if let Some(issued) = relaxation.as_ref().filter(|_| contracts.audit_requested()) {
        relaxation::validate_layout_transition(
            &program.layout,
            issued.record().layout(),
            &program.control_alternatives,
        )
        .map_err(ResolvedLayoutOptimizationError::Relaxation)?;
    }
    if let Some(issued) = &relaxation {
        program.layout = issued.shared_layout();
    }
    let layout = SettledFunctionLayout {
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
    };
    if contracts.audit_requested() {
        relaxation::validate_current_layout_optimization(layout.current(), layout.trace())?;
    }
    Ok(RelaxedLayout { contracts, layout })
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

pub mod entry_layout {
    pub use placement::entry_layout::{
        AdapterCallField, ResolvedBridge, ResolvedCallableEntry, ResolvedForeignAdapter,
        ResolvedPrivateEntry,
    };
}
pub mod program_entry_layout {
    pub use placement::program_entry_layout::{ResolvedHostedProgram, ResolvedPhysicalProgram};
}

/// Entry call and relocation fields have fixed widths. Their offsets survive relaxation.
pub struct FixedEntry<T>(placement::ResolvedEntry<T>);

/// Extracted entry fields cannot replace the layout issued by placement.
///
/// ```compile_fail,E0308
/// fn skip_stage(raw: placement::entry_layout::ResolvedPrivateEntry) {
///     let _ = resolved_layout_to_resolved_layout::retain_fixed_entry(raw);
/// }
/// ```
pub fn retain_fixed_entry<T>(layout: placement::ResolvedEntry<T>) -> FixedEntry<T> {
    FixedEntry(layout)
}

impl<T> FixedEntry<T> {
    pub fn into_layout(self) -> T {
        self.0.into_layout()
    }
}
