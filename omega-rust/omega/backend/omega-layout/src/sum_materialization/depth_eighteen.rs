//! Exact plural depth-eighteen conventional-sum materialization projection.

use super::depth_seventeen::project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthEighteenRecordSumPathsLayoutReport;

/// Project the complete nonempty authored-order set of exact depth-eighteen
/// record chains:
/// `Outer -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums`.
///
/// Each qualifying outer occurrence owns the unchanged plural depth-seventeen
/// report for its exact sixteenth record. One shared memoized reachability walk
/// and one global leaf-occurrence ceiling bound the complete projection.
pub fn project_conventional_record_with_depth_eighteen_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthEighteenRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_eighteen_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

pub(super) fn project_conventional_record_with_depth_eighteen_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthEighteenRecordSumPathsLayoutReport, Diagnostic> {
    project_recursive_record_sum_paths_layout(
        program,
        plan,
        data_symbol,
        reachability,
        "depth-eighteen",
        "sixteenth",
        "depth-seventeen",
        project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout_with_reachability,
    )
}
