use crate::tests::*;

use super::fixture::staged_integer_not_equal_conditional;

#[test]
fn inequality_semantics_cover_less_equal_greater_and_u64_boundaries_on_both_isas() {
    let cases = [
        (7_u64, 9_u64, true),
        (9, 9, false),
        (9, 7, true),
        (0, 0, false),
        (0, u64::MAX, true),
        (u64::MAX, 0, true),
        (u64::MAX, u64::MAX, false),
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_integer_not_equal_conditional(target);
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &staged.selected().plan().functions[0].blocks[0].terminator
        else {
            panic!("fixture must branch on the nonzero comparison predicate")
        };
        for &(left, right, expected_true) in &cases {
            let selected_edge = if left != right {
                when_nonzero.psi_edge
            } else {
                when_zero.psi_edge
            };
            assert_eq!(
                selected_edge == EdgeId::new(19_614).unwrap(),
                expected_true,
                "{left} != {right} on {target:?}"
            );
        }
    }
}
