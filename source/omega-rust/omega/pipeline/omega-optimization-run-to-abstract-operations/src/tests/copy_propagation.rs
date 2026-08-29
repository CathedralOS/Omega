//! Copy-propagation projection custody.

use super::*;

#[test]
fn copy_propagation_projects_shortened_blocks_and_rewritten_edges() {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized =
        project_optimization_run(run(redundant_block_parameter_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert!(
        optimized.plan().functions[0].block_entries[1]
            .parameters
            .is_empty()
    );
    assert!(matches!(
        &optimized.plan().functions[0].operations[1],
        AbstractOperation::Jump { bindings, .. } if bindings.is_empty()
    ));
    assert!(matches!(
        &optimized.plan().functions[0].operations[2],
        AbstractOperation::Return { value, .. } if *value == ValueId::new(1_034).unwrap()
    ));
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    assert_eq!(target.optimized().commits().len(), 1);
}

#[test]
fn copy_propagation_preserves_scalar_call_result_effect_and_custody() {
    let verified = call_result_block_parameter_verified();
    let call_before = verified.unit().functions[0].blocks[0].nodes[0].clone();
    let callee_before = verified.unit().functions[1].clone();
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let optimized = project_optimization_run(run(verified, selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(
        optimized.unit().functions[0].blocks[0].nodes[0],
        call_before
    );
    assert!(matches!(
        optimized.unit().functions[0].blocks[0].nodes[0].operation,
        AbstractOperation::Call {
            result,
            callee,
            ..
        } if result == ValueId::new(1_605).unwrap()
            && callee == MachineId::new(1_602).unwrap()
    ));
    assert!(
        optimized.plan().functions[0].block_entries[1]
            .parameters
            .is_empty()
    );
    assert!(matches!(
        &optimized.plan().functions[0].operations[1],
        AbstractOperation::Jump { bindings, .. } if bindings.is_empty()
    ));
    assert!(matches!(
        &optimized.plan().functions[0].operations[2],
        AbstractOperation::Return { value, .. }
            if *value == ValueId::new(1_605).unwrap()
    ));
    assert_eq!(optimized.unit().functions[1], callee_before);
    assert_eq!(
        optimized
            .pre_physical_manifest()
            .record()
            .source_statistics
            .functions,
        2
    );
    assert_eq!(
        optimized
            .pre_physical_manifest()
            .record()
            .optimized_statistics
            .functions,
        2
    );
}
