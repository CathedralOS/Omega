use super::*;

#[test]
fn sccp_registry_appends_range_pair_comparisons_after_literal_range_rules() {
    let registry =
        registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
    let contracts = registry.contracts().collect::<Vec<_>>();
    assert_eq!(contracts.len(), 39);
    assert_eq!(
        contracts[30].identity(),
        IntegerLessThanRangeConstantRule::contract().identity()
    );
    assert_eq!(
        contracts[31].identity(),
        IntegerLessThanConstantRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[32].identity(),
        IntegerLessOrEqualRangeConstantRule::contract().identity()
    );
    assert_eq!(
        contracts[33].identity(),
        IntegerLessOrEqualConstantRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[34].identity(),
        IntegerEqualRangeConstantRule::contract().identity()
    );
    assert_eq!(
        contracts[35].identity(),
        IntegerEqualConstantRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[36].identity(),
        IntegerEqualRangeRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[37].identity(),
        IntegerLessThanRangeRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[38].identity(),
        IntegerLessOrEqualRangeRangeRule::contract().identity()
    );
    assert!(contracts.iter().all(|contract| contract.pass()
        == OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME)));
}
