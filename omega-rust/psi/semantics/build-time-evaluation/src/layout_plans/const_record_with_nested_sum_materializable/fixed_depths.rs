//! Fixed public names over the shared recursive materialization owner.

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

macro_rules! define_recursive_materialization_depth {
    (
        occurrence = $occurrence:ident,
        materialization = $materialization:ident,
        layout = $layout:ty,
        inner = $inner:ty,
        validate = $validate:ident,
        validate_with_reachability = $validate_with_reachability:ident,
        derive_type = $derive_type:ident,
        derive = $derive:ident,
        inner_validate = $inner_validate:path,
        label = $label:literal,
        fingerprint_domain = $fingerprint_domain:literal
    ) => {
        /// Exact custody for one authored outer-field occurrence at this depth.
        pub type $occurrence = ValidatedConstRecursiveNestedSumOccurrenceMaterialization<$inner>;

        /// Exact custody for the complete authored-order path set at this depth.
        pub type $materialization =
            ValidatedConstRecursiveNestedSumsMaterialization<$layout, $inner>;

        impl $materialization {
            /// Re-resolve every path and replay all retained inner custody.
            pub fn replay_against(
                &self,
                typed: &TypedTrees,
                schema_name: &str,
                path_layout: &$layout,
                value: &BuildTimeValue,
                byte_order: ByteOrder,
            ) -> Result<(), MaterializationDiagnostic> {
                replay_recursive_nested_sums(
                    self,
                    typed,
                    schema_name,
                    path_layout,
                    value,
                    byte_order,
                    Self::replay_against_with_reachability,
                )
            }

            pub(super) fn replay_against_with_reachability(
                &self,
                typed: &TypedTrees,
                schema_name: &str,
                path_layout: &$layout,
                value: &BuildTimeValue,
                byte_order: ByteOrder,
                reachability: &mut SumReachability<'_>,
            ) -> Result<(), MaterializationDiagnostic> {
                replay_recursive_nested_sums_with_reachability(
                    self,
                    typed,
                    schema_name,
                    path_layout,
                    value,
                    byte_order,
                    reachability,
                    $label,
                    $fingerprint_domain,
                    $derive,
                    <$inner>::replay_against_with_reachability,
                    <$inner>::schema_name,
                    <$inner>::value,
                    <$inner>::non_authoritative_materialization_report_fingerprint,
                )
            }

            /// Replay complete retained custody before one atomic image copy.
            pub fn apply(
                &self,
                typed: &TypedTrees,
                destination: &mut [u8],
            ) -> Result<(), MaterializationDiagnostic> {
                apply_recursive_nested_sums(self, typed, destination, $label, Self::replay_against)
            }
        }

        pub fn $validate(
            typed: &TypedTrees,
            schema_name: &str,
            path_layout: &$layout,
            value: &BuildTimeValue,
            byte_order: ByteOrder,
        ) -> Result<$materialization, MaterializationDiagnostic> {
            let mut reachability = SumReachability::new(typed);
            $validate_with_reachability(
                typed,
                schema_name,
                path_layout,
                value,
                byte_order,
                &mut reachability,
            )
        }

        pub(super) fn $validate_with_reachability(
            typed: &TypedTrees,
            schema_name: &str,
            path_layout: &$layout,
            value: &BuildTimeValue,
            byte_order: ByteOrder,
            reachability: &mut SumReachability<'_>,
        ) -> Result<$materialization, MaterializationDiagnostic> {
            validate_recursive_nested_sums_with_reachability(
                typed,
                schema_name,
                path_layout,
                value,
                byte_order,
                reachability,
                $fingerprint_domain,
                $derive,
                <$inner>::non_authoritative_materialization_report_fingerprint,
            )
        }

        type $derive_type = DerivedRecursiveNestedSumsMaterialization<$inner>;

        fn $derive(
            typed: &TypedTrees,
            schema_name: &str,
            path_layout: &$layout,
            value: &BuildTimeValue,
            byte_order: ByteOrder,
            reachability: &mut SumReachability<'_>,
        ) -> Result<$derive_type, MaterializationDiagnostic> {
            derive_recursive_nested_sums_bytes_with_reachability(
                typed,
                schema_name,
                path_layout,
                value,
                byte_order,
                reachability,
                $label,
                $inner_validate,
                <$inner>::bytes,
            )
        }
    };
}

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthSixteenNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization,
    layout = ConventionalDepthSixteenRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_sixteen_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_sixteen_nested_sums_with_reachability,
    derive_type = DerivedDepthSixteenNestedSumsMaterialization,
    derive = derive_depth_sixteen_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_fifteen_nested_sums_with_reachability,
    label = "depth-sixteen",
    fingerprint_domain = b"omega.const-materializable-plural-depth-sixteen-record-sum-paths.v1"
);

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthSeventeenNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization,
    layout = ConventionalDepthSeventeenRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_seventeen_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_seventeen_nested_sums_with_reachability,
    derive_type = DerivedDepthSeventeenNestedSumsMaterialization,
    derive = derive_depth_seventeen_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_sixteen_nested_sums_with_reachability,
    label = "depth-seventeen",
    fingerprint_domain = b"omega.const-materializable-plural-depth-seventeen-record-sum-paths.v1"
);

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthEighteenNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization,
    layout = ConventionalDepthEighteenRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_eighteen_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_eighteen_nested_sums_with_reachability,
    derive_type = DerivedDepthEighteenNestedSumsMaterialization,
    derive = derive_depth_eighteen_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_seventeen_nested_sums_with_reachability,
    label = "depth-eighteen",
    fingerprint_domain = b"omega.const-materializable-plural-depth-eighteen-record-sum-paths.v1"
);

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthNineteenNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization,
    layout = ConventionalDepthNineteenRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_nineteen_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_nineteen_nested_sums_with_reachability,
    derive_type = DerivedDepthNineteenNestedSumsMaterialization,
    derive = derive_depth_nineteen_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_eighteen_nested_sums_with_reachability,
    label = "depth-nineteen",
    fingerprint_domain = b"omega.const-materializable-plural-depth-nineteen-record-sum-paths.v1"
);

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthTwentyNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
    layout = ConventionalDepthTwentyRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_twenty_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability,
    derive_type = DerivedDepthTwentyNestedSumsMaterialization,
    derive = derive_depth_twenty_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_nineteen_nested_sums_with_reachability,
    label = "depth-twenty",
    fingerprint_domain = b"omega.const-materializable-plural-depth-twenty-record-sum-paths.v1"
);

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    layout = ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_twenty_one_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability,
    derive_type = DerivedDepthTwentyOneNestedSumsMaterialization,
    derive = derive_depth_twenty_one_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability,
    label = "depth-twenty-one",
    fingerprint_domain = b"omega.const-materializable-plural-depth-twenty-one-record-sum-paths.v1"
);

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthTwentyTwoNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
    layout = ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_twenty_two_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_twenty_two_nested_sums_with_reachability,
    derive_type = DerivedDepthTwentyTwoNestedSumsMaterialization,
    derive = derive_depth_twenty_two_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability,
    label = "depth-twenty-two",
    fingerprint_domain = b"omega.const-materializable-plural-depth-twenty-two-record-sum-paths.v1"
);

define_recursive_materialization_depth!(
    occurrence = ValidatedConstDepthTwentyThreeNestedSumOccurrenceMaterialization,
    materialization = ValidatedConstRecordWithDepthTwentyThreeNestedSumsMaterialization,
    layout = ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
    inner = ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
    validate = validate_const_materializable_record_with_depth_twenty_three_nested_sums,
    validate_with_reachability =
        validate_const_materializable_record_with_depth_twenty_three_nested_sums_with_reachability,
    derive_type = DerivedDepthTwentyThreeNestedSumsMaterialization,
    derive = derive_depth_twenty_three_nested_sums_bytes_with_reachability,
    inner_validate =
        validate_const_materializable_record_with_depth_twenty_two_nested_sums_with_reachability,
    label = "depth-twenty-three",
    fingerprint_domain =
        b"omega.const-materializable-plural-depth-twenty-three-record-sum-paths.v1"
);
