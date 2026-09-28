//! Selected boundary comparisons bind computation nodes that keep the exact
//! operator use; their published/surviving crash routes stay joined through
//! that use rather than through an invented call or a builtin expression.

use super::checked_source;
use checked_trees::CheckedScalarComputationKind;

#[test]
fn unselected_or_mismatched_comparisons_bind_no_selected_computation() {
    for source in [
        // A builtin integer comparison keeps its ordinary pure expression.
        "machine compare(left: i32, right: i32) -> bool { left == right }",
        // A Boolean boundary equality is not an integer comparison.
        "boundary operator == Comparison::equal(left: bool, right: bool) -> bool
         crashes Trap left;
         machine compare(left: bool, right: bool) -> bool { left == right }",
        // A non-boundary selected operator is not a boundary comparison use.
        "operator == Helpers::equal(left: i32, right: i32) -> bool;
         machine compare(left: i32, right: i32) -> bool { left == right }",
        // A non-comparison spelling never becomes a selected comparison even
        // when a crash-qualified boundary requirement supplies it.
        "boundary operator + Comparison::add(left: i32, right: i32) -> i32
         crashes Trap;
         machine compare(left: i32, right: i32) -> bool { (left + right) == 0 }",
    ] {
        let checked = checked_source(source, false);
        assert!(
            checked
                .facts
                .values
                .scalar_computations
                .nodes
                .iter()
                .all(|(_, node)| !matches!(
                    node.kind,
                    CheckedScalarComputationKind::SelectedComparison { .. }
                )),
            "{source}"
        );
    }
}
