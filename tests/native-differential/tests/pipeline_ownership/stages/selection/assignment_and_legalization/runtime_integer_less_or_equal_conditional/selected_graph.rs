use crate::tests::*;

use super::fixture::staged_integer_less_or_equal_conditional;

#[test]
fn inclusive_runtime_predicate_covers_order_and_u64_boundaries_on_both_isas() {
    let cases = [
        (7_u64, 9_u64, true),
        (9, 9, true),
        (9, 7, false),
        (0, 0, true),
        (0, u64::MAX, true),
        (u64::MAX, 0, false),
        (u64::MAX, u64::MAX, true),
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_less_or_equal_conditional(target);
        let SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        } = &staged.selected().plan().functions[0].blocks[0].terminator
        else {
            panic!("fixture must use the reversed strict-less-than predicate")
        };
        for (left, right, expected_true) in cases {
            let reversed_compare_is_less = right < left;
            let selected_edge = if reversed_compare_is_less {
                when_less.psi_edge
            } else {
                when_not_less.psi_edge
            };
            assert_eq!(
                selected_edge == EdgeId::new(19_414).unwrap(),
                expected_true,
                "{left} <= {right} on {target:?}"
            );
        }
    }
}
