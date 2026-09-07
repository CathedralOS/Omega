//! Source, proof, fuel and selected branch replay use the ordinary graph.
use super::fixture::staged_integer_not_equal_conditional;
use crate::tests::*;
#[test]
fn ordinary_condition_graph_rejects_detached_custody() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert_ordinary_graph_custody(&staged_integer_not_equal_conditional(target));
    }
}
