//! Exact SCCP catalog positions for Boolean-result constant evaluation.

use super::*;

#[test]
fn sccp_registry_places_boolean_result_constant_rules_before_range_rules() {
    let registry =
        registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
    let contracts = registry.contracts().collect::<Vec<_>>();
    assert_eq!(contracts.len(), 39);
    assert_eq!(
        contracts[25].identity(),
        BooleanNotConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[26].identity(),
        BooleanEqualConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[27].identity(),
        IntegerEqualConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[28].identity(),
        IntegerLessThanConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[29].identity(),
        IntegerLessOrEqualConstantsRule::contract().identity()
    );
}
