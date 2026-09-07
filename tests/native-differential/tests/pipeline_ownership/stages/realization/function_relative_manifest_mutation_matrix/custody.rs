//! Independent direct function-relative realization receipt-root mutations.

use crate::tests::*;

use super::fixture::{alternate_direct_rel8_realization, direct_rel8_realization};

#[test]
fn every_direct_function_relative_receipt_root_rejects_independently() {
    let donor = alternate_direct_rel8_realization();
    for field in [
        FixedFramePublicationCustodyFieldForTest::Source,
        FixedFramePublicationCustodyFieldForTest::Machine,
        FixedFramePublicationCustodyFieldForTest::Frame,
        FixedFramePublicationCustodyFieldForTest::Protocol,
        FixedFramePublicationCustodyFieldForTest::ExitContract,
        FixedFramePublicationCustodyFieldForTest::Realization,
    ] {
        let mut staged = direct_rel8_realization();
        staged.corrupt_publication_custody_for_test(field, &donor);
        assert_eq!(
            validate_fixed_frame_function_relative_realization(&staged),
            Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch),
            "mutated {field:?} receipt root must fail independent replay",
        );
    }
}
