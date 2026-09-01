//! Registry dispatch and propagated-fact constant evaluation.

use super::*;

#[test]
fn selected_builtin_proposes_one_independently_validated_exact_fold() {
    let unit = exact_add_unit();
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
    let products = vec![constants, ranges];
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    assert_eq!(registry.len(), 39);
    let mut dispatched = 0usize;
    let mut candidates = Vec::new();
    for rule in registry.iter() {
        dispatched += 1;
        candidates.extend(
            rule.propose(&unit, RuleAnalysisView::new(&products))
                .unwrap(),
        );
    }
    assert_eq!(dispatched, registry.len());
    assert_eq!(candidates.len(), 1);
    let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(15),
            ..
        }
    ));
}

#[test]
fn propagated_block_parameter_fact_is_independently_reconstructed() {
    let unit = propagated_block_parameter_unit(true);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    let candidates = IntegerBitwiseNotConstantsRule
        .propose(&unit, RuleAnalysisView::new(&[constants]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[3].nodes[0].operation,
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(248),
            ..
        }
    ));
}
