//! Exact plural depth-nineteen constant-value custody and replay.

use super::depth_eighteen::validate_const_materializable_record_with_depth_eighteen_nested_sums_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthNineteenRecordSumPathsLayoutReport;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-nineteen path set.
///
/// The nested carrier retains its complete plural depth-eighteen custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthNineteenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-eighteen carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthNineteenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-eighteen carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthNineteenRecordSumPathsLayoutReport,
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
        path_layout: &ConventionalDepthNineteenRecordSumPathsLayoutReport,
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
            "depth-nineteen",
            b"omega.const-materializable-plural-depth-nineteen-record-sum-paths.v1",
            derive_depth_nineteen_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-nineteen",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_nineteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_nineteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_nineteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-nineteen-record-sum-paths.v1",
        derive_depth_nineteen_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthNineteenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization,
>;

fn derive_depth_nineteen_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthNineteenNestedSumsMaterialization, MaterializationDiagnostic> {
    derive_recursive_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        "depth-nineteen",
        validate_const_materializable_record_with_depth_eighteen_nested_sums_with_reachability,
        ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization::bytes,
    )
}
