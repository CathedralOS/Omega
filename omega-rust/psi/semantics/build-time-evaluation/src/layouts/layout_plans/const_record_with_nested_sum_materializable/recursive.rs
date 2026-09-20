//! Recursive record-level custody over the report's single `children`
//! channel — the same exact custody at every record boundary.

use super::{
    BuildTimeValue, ByteOrder, ConventionalRecordSumChildInterior,
    ConventionalRecursiveRecordSumPathsLayoutReport, MaterializationDiagnostic, SumReachability,
    TypedTrees, ValidatedConstRecordSumChildMaterialization,
    child_materializations_match_for_replay, derive_record_level_children_bytes,
    field_occurrence_matches, normalized_layout_plan_report_fingerprint,
    record_sum_paths_reports_match_for_replay, recursive_level_materialization_report_fingerprint,
};
#[cfg(test)]
mod tests;

/// Complete value-sensitive custody for one recursive record level: the
/// retained recursive report beside the level's authored-order child
/// custody rows and complete staged bytes.
///
/// The retained `path_layout` holds every supplied child row — its own path
/// segment and child report — so replay compares hash-free layout facts
/// before re-deriving the value-sensitive custody. Record children keep
/// their own complete recursive custody; record-array children keep only the
/// compact per-element selections because the shared element report lives
/// once in the retained `path_layout` row. This type does not implement
/// `Clone`: replay reconstructs every outer and nested fact from the
/// caller's current typed program.
#[derive(Debug)]
pub struct ValidatedConstRecordWithRecursiveNestedSumsMaterialization {
    pub(crate) schema_name: String,
    pub(crate) non_authoritative_schema_report_fingerprint: u64,
    pub(crate) value: BuildTimeValue,
    pub(crate) path_layout: ConventionalRecursiveRecordSumPathsLayoutReport,
    pub(crate) non_authoritative_outer_layout_report_fingerprint: u64,
    pub(crate) children: Vec<ValidatedConstRecordSumChildMaterialization>,
    pub(crate) byte_order: ByteOrder,
    pub(crate) bytes: Vec<u8>,
    pub(crate) non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithRecursiveNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    /// The level's retained recursive report — every supplied child row in
    /// authored field order.
    pub const fn path_layout(&self) -> &ConventionalRecursiveRecordSumPathsLayoutReport {
        &self.path_layout
    }

    /// The level's direct-child custody in authored field order — each row
    /// carries the custody matching its own path segment.
    pub fn children(&self) -> &[ValidatedConstRecordSumChildMaterialization] {
        &self.children
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Independently rederive the typed value, path geometry, and complete bytes.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalRecursiveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_report_resources(path_layout)?;
        let mut reachability = SumReachability::new(typed);
        self.replay_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            &mut reachability,
        )
    }

    pub(super) fn replay_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalRecursiveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable recursive invocation drifted from retained custody".to_owned(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !record_sum_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable recursive layout drifted from retained custody".to_owned(),
            ));
        }

        let replayed = derive_record_level_children_bytes(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.children.len() != self.children.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable recursive custody changed cardinality".to_owned(),
            ));
        }
        for (((retained_child, replayed_child), supplied_row), retained_row) in self
            .children
            .iter()
            .zip(&replayed.children)
            .zip(&path_layout.children)
            .zip(&self.path_layout.children)
        {
            if !child_materializations_match_for_replay(retained_child, replayed_child)
                || !field_occurrence_matches(
                    &supplied_row.field,
                    supplied_row.member_identity,
                    &retained_row.field,
                    retained_row.member_identity,
                )
            {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable recursive child custody drifted from retained custody"
                        .to_owned(),
                ));
            }
            // A record child's inner custody replays the deeper level
            // pairwise before its fingerprint compares.
            if let (
                ValidatedConstRecordSumChildMaterialization::Record(retained_occurrence),
                ValidatedConstRecordSumChildMaterialization::Record(replayed_occurrence),
                ConventionalRecordSumChildInterior::Record(supplied_inner),
            ) = (retained_child, replayed_child, &supplied_row.interior)
            {
                retained_occurrence.inner.replay_with_reachability(
                    typed,
                    replayed_occurrence.inner.schema_name(),
                    supplied_inner,
                    replayed_occurrence.inner.value(),
                    byte_order,
                    reachability,
                )?;
                if retained_occurrence
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
                    != replayed_occurrence
                        .inner
                        .non_authoritative_materialization_report_fingerprint()
                {
                    return Err(MaterializationDiagnostic(
                        "ConstMaterializable recursive inner custody drifted after exact replay"
                            .to_owned(),
                    ));
                }
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable recursive bytes drifted after exact replay".to_owned(),
            ));
        }
        let fingerprint = recursive_level_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.children,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable recursive fingerprint drifted after exact replay".to_owned(),
            ));
        }
        Ok(())
    }

    /// Replay every retained child row before one atomic copy of the level's
    /// complete image.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        let mut reachability = SumReachability::new(typed);
        self.replay_with_reachability(
            typed,
            &self.schema_name,
            &self.path_layout,
            &self.value,
            self.byte_order,
            &mut reachability,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable recursive copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Validate complete recursive paths before granting materialization custody.
pub fn validate_const_materializable_record_with_recursive_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalRecursiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithRecursiveNestedSumsMaterialization, MaterializationDiagnostic> {
    validate_report_resources(path_layout)?;
    let mut reachability = SumReachability::new(typed);
    validate_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalRecursiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithRecursiveNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_record_level_children_bytes(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = recursive_level_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.children,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithRecursiveNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        children: derived.children,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

fn validate_report_resources(
    report: &ConventionalRecursiveRecordSumPathsLayoutReport,
) -> Result<(), MaterializationDiagnostic> {
    let mut pending = vec![(report, 1usize)];
    let mut occurrences = 0usize;
    while let Some((report, depth)) = pending.pop() {
        if depth > layout_plans::CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
            return Err(MaterializationDiagnostic("ConstMaterializable recursive record paths exceed the compiler depth resource bound of 64".into()));
        }
        let record_children = report
            .children
            .iter()
            .filter(|child| {
                matches!(
                    child.interior,
                    ConventionalRecordSumChildInterior::Record(_)
                )
            })
            .count();
        pending.try_reserve(record_children).map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable recursive traversal exceeds compiler resources".into(),
            )
        })?;
        pending.extend(
            report
                .children
                .iter()
                .filter_map(|child| match &child.interior {
                    ConventionalRecordSumChildInterior::Record(inner) => Some((inner, depth + 1)),
                    ConventionalRecordSumChildInterior::Sum(_) => None,
                }),
        );
        occurrences = occurrences
            .checked_add(report.children.len())
            .ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable recursive occurrence count overflows".into(),
                )
            })?;
        if occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable recursive paths exceed the global occurrence resource bound"
                    .into(),
            ));
        }
    }
    Ok(())
}
