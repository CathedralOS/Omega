//! Recursive dispatch retains the same exact custody at every record boundary.

use super::{
    BuildTimeValue, ByteOrder, ConventionalRecursiveRecordSumPathsLayoutReport,
    MaterializationDiagnostic, SumReachability, TypedTrees,
    ValidatedConstRecordLevelSumChildrenMaterialization,
    ValidatedConstRecursiveNestedSumsMaterialization,
    replay_recursive_nested_sums_with_reachability,
    validate_record_level_sum_children_with_reachability,
    validate_recursive_nested_sums_with_reachability,
};
#[cfg(test)]
mod tests;

/// Complete value-sensitive custody, with nesting represented by occurrences.
#[derive(Debug)]
pub enum ValidatedConstRecordWithRecursiveNestedSumsMaterialization {
    Leaf(ValidatedConstRecordLevelSumChildrenMaterialization),
    Branch(ValidatedConstRecursiveNestedSumsMaterialization),
}

impl ValidatedConstRecordWithRecursiveNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        match self {
            Self::Leaf(custody) => custody.schema_name(),
            Self::Branch(custody) => custody.schema_name(),
        }
    }

    pub fn value(&self) -> &BuildTimeValue {
        match self {
            Self::Leaf(custody) => custody.value(),
            Self::Branch(custody) => custody.value(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Leaf(custody) => custody.bytes(),
            Self::Branch(custody) => custody.bytes(),
        }
    }

    pub fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        match self {
            Self::Leaf(custody) => custody.non_authoritative_materialization_report_fingerprint(),
            Self::Branch(custody) => custody.non_authoritative_materialization_report_fingerprint(),
        }
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
        match (self, path_layout) {
            (
                Self::Leaf(custody),
                ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
                    outer_layout,
                    child_sum_layouts,
                    child_sum_array_layouts,
                },
            ) => custody.replay_against_with_reachability(
                typed,
                schema_name,
                outer_layout,
                child_sum_layouts,
                child_sum_array_layouts,
                value,
                byte_order,
                reachability,
            ),
            (
                Self::Branch(custody),
                ConventionalRecursiveRecordSumPathsLayoutReport::Branch(report),
            ) => replay_recursive_nested_sums_with_reachability(
                custody,
                typed,
                schema_name,
                report,
                value,
                byte_order,
                reachability,
            ),
            _ => Err(MaterializationDiagnostic(
                "ConstMaterializable recursive path leaf/branch drifted from retained custody"
                    .into(),
            )),
        }
    }

    /// Replay before copying; every failure leaves the destination unchanged.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        match self {
            Self::Leaf(custody) => custody.apply(typed, destination),
            Self::Branch(custody) => {
                let mut reachability = SumReachability::new(typed);
                replay_recursive_nested_sums_with_reachability(
                    custody,
                    typed,
                    &custody.schema_name,
                    &custody.path_layout,
                    &custody.value,
                    custody.byte_order,
                    &mut reachability,
                )?;
                if destination.len() < custody.bytes.len() {
                    return Err(MaterializationDiagnostic(format!(
                        "ConstMaterializable recursive copy needs {} bytes, destination has {}",
                        custody.bytes.len(),
                        destination.len()
                    )));
                }
                destination[..custody.bytes.len()].copy_from_slice(&custody.bytes);
                Ok(())
            }
        }
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
    match path_layout {
        ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
            outer_layout,
            child_sum_layouts,
            child_sum_array_layouts,
        } => validate_record_level_sum_children_with_reachability(
            typed,
            schema_name,
            outer_layout,
            child_sum_layouts,
            child_sum_array_layouts,
            value,
            byte_order,
            reachability,
        )
        .map(ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Leaf),
        ConventionalRecursiveRecordSumPathsLayoutReport::Branch(report) => {
            validate_recursive_nested_sums_with_reachability(
                typed,
                schema_name,
                report,
                value,
                byte_order,
                reachability,
            )
            .map(ValidatedConstRecordWithRecursiveNestedSumsMaterialization::Branch)
        }
    }
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
        let count = match report {
            ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
                child_sum_layouts,
                child_sum_array_layouts,
                ..
            } => child_sum_layouts
                .len()
                .checked_add(child_sum_array_layouts.len())
                .ok_or_else(|| {
                    MaterializationDiagnostic(
                        "ConstMaterializable recursive occurrence count overflows".into(),
                    )
                })?,
            ConventionalRecursiveRecordSumPathsLayoutReport::Branch(report) => {
                pending.try_reserve(report.paths.len()).map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable recursive traversal exceeds compiler resources".into(),
                    )
                })?;
                pending.extend(report.paths.iter().map(|path| (&path.inner, depth + 1)));
                report
                    .paths
                    .len()
                    .checked_add(report.child_sum_layouts.len())
                    .and_then(|count| count.checked_add(report.child_sum_array_layouts.len()))
                    .ok_or_else(|| {
                        MaterializationDiagnostic(
                            "ConstMaterializable recursive occurrence count overflows".into(),
                        )
                    })?
            }
        };
        occurrences = occurrences.checked_add(count).ok_or_else(|| {
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
