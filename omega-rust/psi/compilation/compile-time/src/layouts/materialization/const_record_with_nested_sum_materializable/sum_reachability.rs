//! Const-materialization's view of the shared conventional-sum reachability
//! walk in `typed_trees`: same bounded engine, re-stamped into
//! `MaterializationDiagnostic` under this rung's `ConstMaterializable
//! nested-record` label so reports keep their authored prefix.

use super::{DataDefinition, MaterializationDiagnostic, TypedTrees};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) use typed_trees::RecordSumProfile;

const REACHABILITY_LABEL: &str = "ConstMaterializable nested-record";
const FIELD_LABEL: &str = "named field type";

/// The const-materialization reachability walk: the shared
/// [`typed_trees::SumReachability`] engine stamped with this rung's label.
pub(in crate::layouts::materialization) struct SumReachability<'a> {
    inner: typed_trees::SumReachability<'a>,
}

impl<'a> SumReachability<'a> {
    pub(in crate::layouts::materialization) const MAX_EDGES: usize =
        typed_trees::SumReachability::MAX_EDGES;

    pub(in crate::layouts::materialization) fn new(typed: &'a TypedTrees) -> Self {
        Self {
            inner: typed_trees::SumReachability::new(typed, REACHABILITY_LABEL, FIELD_LABEL),
        }
    }

    pub(in crate::layouts::materialization) fn type_contains_sum(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> Result<bool, MaterializationDiagnostic> {
        self.inner
            .type_contains_sum(type_reference)
            .map_err(|fault| MaterializationDiagnostic(fault.0))
    }
}

/// Classify `data`'s relevant fields against `reachability`'s bounded walk.
pub(super) fn record_sum_profile(
    typed: &TypedTrees,
    data: &DataDefinition,
    reachability: &mut SumReachability<'_>,
) -> Result<RecordSumProfile, MaterializationDiagnostic> {
    typed_trees::record_sum_profile(typed, data, &mut reachability.inner)
        .map_err(|fault| MaterializationDiagnostic(fault.0))
}

pub(super) fn reject_sum_array_type(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    path: &str,
    reachability: &mut SumReachability<'_>,
) -> Result<(), MaterializationDiagnostic> {
    if matches!(
        typed.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::FixedArray { .. }
    ) && reachability.type_contains_sum(type_reference)?
    {
        return Err(MaterializationDiagnostic(format!(
            "{path} uses an array containing sums, outside the nested-record path rung"
        )));
    }
    Ok(())
}
