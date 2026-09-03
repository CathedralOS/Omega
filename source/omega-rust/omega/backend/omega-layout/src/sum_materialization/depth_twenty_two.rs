//! Exact plural depth-twenty-two conventional-sum materialization projection.

use super::depth_twenty_one::project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthTwentyTwoRecordSumPathsLayoutReport;

/// Project the complete nonempty authored-order set of exact depth-twenty-two
/// record chains:
/// `Outer -> Twentieth -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums`.
///
/// Each qualifying outer occurrence owns the unchanged plural depth-twenty-one
/// report for its exact twentieth record. One shared memoized reachability walk
/// and one global leaf-occurrence ceiling bound the complete projection.
pub fn project_conventional_record_with_depth_twenty_two_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthTwentyTwoRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_twenty_two_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

pub(super) fn project_conventional_record_with_depth_twenty_two_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthTwentyTwoRecordSumPathsLayoutReport, Diagnostic> {
    project_recursive_record_sum_paths_layout(
        program,
        plan,
        data_symbol,
        reachability,
        "depth-twenty-two",
        "twentieth",
        "depth-twenty-one",
        project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout_with_reachability,
    )
}
