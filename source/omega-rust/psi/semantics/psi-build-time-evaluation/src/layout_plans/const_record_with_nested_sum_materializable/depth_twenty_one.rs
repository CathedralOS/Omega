//! Exact plural depth-twenty-one constant-value custody and replay.

use super::depth_twenty::validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthTwentyOneRecordSumPathsLayoutReport;

fn depth_twenty_one_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-twenty-one-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-twenty-one path set.
///
/// The nested carrier retains its complete plural depth-twenty custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-twenty carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTwentyOneRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-twenty carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        let mut reachability = SumReachability::new(typed);
        self.replay_against_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            &mut reachability,
        )
    }

    pub(super) fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !record_sum_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_twenty_one_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one custody changed cardinality".into(),
            ));
        }
        for (((retained, replayed), path), retained_path) in self
            .occurrences
            .iter()
            .zip(&replayed.occurrences)
            .zip(&path_layout.paths)
            .zip(&self.path_layout.paths)
        {
            if !field_occurrence_matches(
                retained.outer_field(),
                retained.outer_member_identity(),
                replayed.outer_field(),
                replayed.outer_member_identity(),
            ) || !field_occurrence_matches(
                &path.outer_field,
                path.outer_member_identity,
                &retained_path.outer_field,
                retained_path.outer_member_identity,
            ) {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable plural depth-twenty-one occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.inner,
                replayed.inner.value(),
                byte_order,
                reachability,
            )?;
            if retained
                .inner
                .non_authoritative_materialization_report_fingerprint()
                != replayed
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
            {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable plural depth-twenty-one inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one bytes drifted after exact replay"
                    .into(),
            ));
        }
        let fingerprint = depth_twenty_one_nested_sums_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.occurrences,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one fingerprint drifted after exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.path_layout,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-twenty-one copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_twenty_one_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    let derived = derive_depth_twenty_one_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint =
        depth_twenty_one_nested_sums_materialization_report_fingerprint(
            schema_name,
            derived.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &derived.occurrences,
            byte_order,
            value,
            &derived.bytes,
        );
    Ok(
        ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization {
            schema_name: schema_name.to_owned(),
            non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
            value: value.clone(),
            path_layout: path_layout.clone(),
            non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
            occurrences: derived.occurrences,
            byte_order,
            bytes: derived.bytes,
            non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
        },
    )
}

type DerivedDepthTwentyOneNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
>;

fn derive_depth_twenty_one_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTwentyOneNestedSumsMaterialization, MaterializationDiagnostic> {
    derive_recursive_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        "depth-twenty-one",
        validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability,
        ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization::bytes,
    )
}
