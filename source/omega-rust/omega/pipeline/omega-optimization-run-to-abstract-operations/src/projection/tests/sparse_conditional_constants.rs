use super::*;

#[test]
fn proof_certified_exact_fold_projects_and_remains_target_lowerable() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let optimized = project_optimization_run(run(exact_add_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert!(matches!(
        optimized.plan().functions[0].operations[2],
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(15),
            ..
        }
    ));
    assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    assert_eq!(target.target(), NativeTarget::linux_x64());
    assert_eq!(target.optimized().commits().len(), 1);
    assert_eq!(target.target_operations().functions.len(), 1);
}
