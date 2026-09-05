//! Fixed public names over the shared recursive layout projection.

use super::*;
use layout_plans::{
    ConventionalDepthEighteenRecordSumPathsLayoutReport,
    ConventionalDepthNineteenRecordSumPathsLayoutReport,
    ConventionalDepthSeventeenRecordSumPathsLayoutReport,
    ConventionalDepthSixteenRecordSumPathsLayoutReport,
    ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    ConventionalDepthTwentyRecordSumPathsLayoutReport,
    ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
    ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
};

macro_rules! define_recursive_projection_depth {
    (
        report = $report:ty,
        project = $project:ident,
        project_with_reachability = $project_with_reachability:ident,
        inner_project = $inner_project:path,
        depth = $depth:literal,
        inner_field = $inner_field:literal,
        inner_depth = $inner_depth:literal
    ) => {
        /// Project the complete nonempty authored-order path set at this depth.
        pub fn $project(
            program: &CheckedTrees,
            plan: &LayoutPlan,
            data_symbol: SymbolHandle,
        ) -> Result<$report, Diagnostic> {
            let mut reachability = SumReachability::new(program);
            $project_with_reachability(program, plan, data_symbol, &mut reachability)
        }

        pub(super) fn $project_with_reachability(
            program: &CheckedTrees,
            plan: &LayoutPlan,
            data_symbol: SymbolHandle,
            reachability: &mut SumReachability<'_>,
        ) -> Result<$report, Diagnostic> {
            project_recursive_record_sum_paths_layout(
                program,
                plan,
                data_symbol,
                reachability,
                $depth,
                $inner_field,
                $inner_depth,
                $inner_project,
            )
        }
    };
}

define_recursive_projection_depth!(
    report = ConventionalDepthSixteenRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_fifteen_nested_sums_materialization_layout_with_reachability,
    depth = "depth-sixteen",
    inner_field = "fourteenth",
    inner_depth = "depth-fifteen"
);

define_recursive_projection_depth!(
    report = ConventionalDepthSeventeenRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_sixteen_nested_sums_materialization_layout_with_reachability,
    depth = "depth-seventeen",
    inner_field = "fifteenth",
    inner_depth = "depth-sixteen"
);

define_recursive_projection_depth!(
    report = ConventionalDepthEighteenRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_eighteen_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_eighteen_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_seventeen_nested_sums_materialization_layout_with_reachability,
    depth = "depth-eighteen",
    inner_field = "sixteenth",
    inner_depth = "depth-seventeen"
);

define_recursive_projection_depth!(
    report = ConventionalDepthNineteenRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_nineteen_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_nineteen_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_eighteen_nested_sums_materialization_layout_with_reachability,
    depth = "depth-nineteen",
    inner_field = "seventeenth",
    inner_depth = "depth-eighteen"
);

define_recursive_projection_depth!(
    report = ConventionalDepthTwentyRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_twenty_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_twenty_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_nineteen_nested_sums_materialization_layout_with_reachability,
    depth = "depth-twenty",
    inner_field = "eighteenth",
    inner_depth = "depth-nineteen"
);

define_recursive_projection_depth!(
    report = ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_twenty_nested_sums_materialization_layout_with_reachability,
    depth = "depth-twenty-one",
    inner_field = "nineteenth",
    inner_depth = "depth-twenty"
);

define_recursive_projection_depth!(
    report = ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_twenty_two_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_twenty_two_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_twenty_one_nested_sums_materialization_layout_with_reachability,
    depth = "depth-twenty-two",
    inner_field = "twentieth",
    inner_depth = "depth-twenty-one"
);

define_recursive_projection_depth!(
    report = ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
    project = project_conventional_record_with_depth_twenty_three_nested_sums_materialization_layout,
    project_with_reachability = project_conventional_record_with_depth_twenty_three_nested_sums_materialization_layout_with_reachability,
    inner_project = project_conventional_record_with_depth_twenty_two_nested_sums_materialization_layout_with_reachability,
    depth = "depth-twenty-three",
    inner_field = "twenty-first",
    inner_depth = "depth-twenty-two"
);
