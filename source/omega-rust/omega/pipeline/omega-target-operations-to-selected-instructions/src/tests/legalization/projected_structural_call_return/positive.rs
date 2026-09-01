use super::*;

#[test]
fn exact_projected_closure_legalizes_and_receipts_on_all_targets() {
    for target_profile in targets() {
        let (source, target, unit) = projected_fixture(target_profile);
        let legalized = legalize_target_operations(&target, &source, &unit)
            .expect("the exact closure identity-legalizes");
        assert!(legalized.plan().functions.is_empty());
        assert!(legalized.plan().unit_functions.is_empty());
        assert!(legalized.plan().structural_unit_functions.is_empty());
        let [closure] = legalized
            .plan()
            .projected_structural_call_returns
            .as_slice()
        else {
            panic!("one atomic caller/callee closure")
        };
        assert_eq!(closure.caller, target.functions[0]);
        assert_eq!(closure.callee, target.functions[1]);
        let receipt = legalized
            .receipt()
            .projected_structural_call_return()
            .expect("typed projected-roster receipt");
        assert_eq!(receipt.caller(), target.functions[0].machine);
        assert_eq!(receipt.callee(), target.functions[1].machine);
        assert_eq!(receipt.projected_qualification_count(), 2);
        assert_eq!(legalized.receipt().function_count(), 2);
    }
}
