use super::*;
use optimization_core::{Optimization, OptimizationSelections};

#[test]
fn retired_physical_rewrites_reject_instead_of_selecting_another_emitter() {
    let selections = PostTerminalOptimizationSelections::new(
        OptimizationSelections::new([Optimization::X86SelectXorZeroI64MaterializationV1]).unwrap(),
    )
    .unwrap();
    assert!(
        matches!(validate_physical_selections(&selections, target::Architecture::X86_64),
        Err(OptimizedVerifiedPhysicalPipelineError::UnconsumedPostTerminalPhase(actual)) if actual == OptimizationExecutionPhase::PostAllocationMachine)
    );
}

#[test]
fn selected_lowering_catalog_selections_pass_the_physical_gate() {
    for optimization in [
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        Optimization::SelectedIncomingU12CompareImmediate,
    ] {
        let selections = PostTerminalOptimizationSelections::new(
            OptimizationSelections::new([optimization]).unwrap(),
        )
        .unwrap();
        validate_physical_selections(&selections, target::Architecture::X86_64).unwrap_or_else(
            |error| panic!("{optimization:?} must pass the physical gate: {error:?}"),
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
