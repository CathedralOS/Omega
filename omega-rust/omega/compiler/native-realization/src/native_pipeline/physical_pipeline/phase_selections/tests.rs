use super::*;
use optimization_core::{Optimization, OptimizationSelections};

#[test]
fn retired_physical_rewrites_reject_instead_of_selecting_another_emitter() {
    for (optimization, phase) in [
        (
            Optimization::SelectedIncomingU12ExactAddImmediate,
            OptimizationExecutionPhase::SelectedLowering,
        ),
        (
            Optimization::X86SelectXorZeroI64MaterializationV1,
            OptimizationExecutionPhase::PostAllocationMachine,
        ),
    ] {
        let selections = PostTerminalOptimizationSelections::new(
            OptimizationSelections::new([optimization]).unwrap(),
        )
        .unwrap();
        assert!(
            matches!(validate_physical_selections(&selections, target::Architecture::X86_64),
            Err(OptimizedVerifiedPhysicalPipelineError::UnconsumedPostTerminalPhase(actual)) if actual == phase)
        );
    }
}

#[test]
fn canonical_frame_accepts_empty_and_layout_selection_without_route_selection() {
    validate_physical_selections(
        &PostTerminalOptimizationSelections::default(),
        target::Architecture::X86_64,
    )
    .unwrap();
    let selections = PostTerminalOptimizationSelections::new(
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap(),
    )
    .unwrap();
    validate_physical_selections(&selections, target::Architecture::X86_64).unwrap();
    assert!(matches!(
        validate_physical_selections(&selections, target::Architecture::Aarch64),
        Err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeLayoutRuleCatalog(_))
    ));
}
