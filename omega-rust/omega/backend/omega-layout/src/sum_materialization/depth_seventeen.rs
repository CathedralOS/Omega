//! Exact plural depth-seventeen conventional-sum materialization projection.

use super::depth_sixteen::project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthSeventeenRecordSumPathsLayoutReport;

/// Project the complete nonempty authored-order set of exact depth-seventeen
/// record chains:
/// `Outer -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums`.
///
/// Each qualifying outer occurrence owns the unchanged plural depth-sixteen
/// report for its exact fifteenth record. One shared memoized reachability walk
/// and one global leaf-occurrence ceiling bound the complete projection.
pub fn project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalDepthSeventeenRecordSumPathsLayoutReport, Diagnostic> {
    let mut reachability = SumReachability::new(program);
    project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout_with_reachability(
        program,
        plan,
        data_symbol,
        &mut reachability,
    )
}

pub(super) fn project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout_with_reachability(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
    reachability: &mut SumReachability<'_>,
) -> Result<ConventionalDepthSeventeenRecordSumPathsLayoutReport, Diagnostic> {
    project_recursive_record_sum_paths_layout(
        program,
        plan,
        data_symbol,
        reachability,
        "depth-seventeen",
        "fifteenth",
        "depth-sixteen",
        project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout_with_reachability,
    )
}
