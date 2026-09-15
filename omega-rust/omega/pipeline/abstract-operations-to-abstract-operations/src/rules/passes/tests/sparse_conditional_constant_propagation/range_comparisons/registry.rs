use super::super::super::super::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule, IntegerEqualRangeRangeRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessOrEqualRangeRangeRule, IntegerLessThanConstantRangeRule,
    IntegerLessThanRangeConstantRule, IntegerLessThanRangeRangeRule, SCCP_PASS_NAME,
};
use super::super::super::{OptimizationPassIdentity, PsiOptimization, registry_for_optimization};

#[test]
fn sccp_registry_appends_range_pair_comparisons_after_literal_range_rules() {
    let registry =
        registry_for_optimization(PsiOptimization::SparseConditionalConstantPropagation).unwrap();
    let contracts = registry.contracts().collect::<Vec<_>>();
    assert_eq!(contracts.len(), 39);
    let expected_range_contracts = [
        IntegerLessThanRangeConstantRule::contract(),
        IntegerLessThanConstantRangeRule::contract(),
        IntegerLessOrEqualRangeConstantRule::contract(),
        IntegerLessOrEqualConstantRangeRule::contract(),
        IntegerEqualRangeConstantRule::contract(),
        IntegerEqualConstantRangeRule::contract(),
        IntegerEqualRangeRangeRule::contract(),
        IntegerLessThanRangeRangeRule::contract(),
        IntegerLessOrEqualRangeRangeRule::contract(),
    ];
    assert_eq!(&contracts[30..=38], &expected_range_contracts);
    assert!(contracts.iter().all(|contract| contract.pass()
        == OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME)));
}
