//! Independent direct function-relative realization receipt-root mutations.

use crate::tests::*;

use super::fixture::{alternate_direct_rel8_realization, direct_rel8_realization};

#[test]
fn every_direct_function_relative_receipt_root_rejects_independently() {
    let donor = alternate_direct_rel8_realization();
    for field in [
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Source,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Machine,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Relaxation,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::ExitContract,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Realization,
    ] {
        let mut staged = direct_rel8_realization();
        staged.corrupt_publication_custody_for_test(field, &donor);
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(&staged),
            Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch),
            "mutated {field:?} receipt root must fail independent replay",
        );
    }
}
