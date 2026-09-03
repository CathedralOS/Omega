//! Value-sensitive materialization of record fields containing records with
//! direct conventional pure-sum fields.

use psi_layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder,
    ConventionalDepthEightRecordSumPathsLayoutReport,
    ConventionalDepthElevenRecordSumPathsLayoutReport,
    ConventionalDepthFifteenRecordSumPathsLayoutReport,
    ConventionalDepthFiveRecordSumPathsLayoutReport,
    ConventionalDepthFourRecordSumPathsLayoutReport,
    ConventionalDepthFourteenRecordSumPathsLayoutReport,
    ConventionalDepthNineRecordSumPathsLayoutReport,
    ConventionalDepthSevenRecordSumPathsLayoutReport,
    ConventionalDepthSixRecordSumPathsLayoutReport, ConventionalDepthTenRecordSumPathsLayoutReport,
    ConventionalDepthThirteenRecordSumPathsLayoutReport,
    ConventionalDepthThreeRecordSumPathLayoutReport,
    ConventionalDepthThreeRecordSumPathsLayoutReport,
    ConventionalDepthTwelveRecordSumPathsLayoutReport,
    ConventionalDepthTwoRecordSumPathLayoutReport, ConventionalDepthTwoRecordSumPathsLayoutReport,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalNestedRecordSumPathsLayoutReport,
    MaterializationDiagnostic, conventional_sum_layout_reports_match_for_replay,
    layout_plan_reports_match_for_replay, materialize_aggregate_layout_into,
    normalized_layout_plan_report_fingerprint,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use psi_typed_trees::types::TypeReferenceNode;

use super::const_materializable::{
    hash_byte, hash_bytes, hash_text, hash_u64, hash_value, unique_data_by_name, validate_value,
    value_kind,
};
use super::const_record_with_sum_materializable::{
    EncodedOuterField, exact_named_data, field_occurrence_matches, validate_outer_layout,
    validate_outer_record_owner,
};
use super::{
    BuildTimeValue, encode_typed_owned_value, exact_struct_fields,
    normalized_schema_report_fingerprint, reflected_field_layout,
    validate_const_materializable_record_with_conventional_sums,
};
use crate::layout_plans::ValidatedConstRecordWithSumMaterialization;

mod depth_sixteen;
mod depth_seventeen;
mod depth_eighteen;
mod depth_nineteen;
mod depth_twenty;
mod depth_twenty_one;
mod depth_twenty_two;
mod depth_twenty_three;
mod derivation;
mod report_identity;
mod sum_reachability;

pub use depth_sixteen::{
    ValidatedConstDepthSixteenNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_sixteen_nested_sums,
};
pub use depth_seventeen::{
    ValidatedConstDepthSeventeenNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_seventeen_nested_sums,
};
pub use depth_eighteen::{
    ValidatedConstDepthEighteenNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_eighteen_nested_sums,
};
pub use depth_nineteen::{
    ValidatedConstDepthNineteenNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthNineteenNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_nineteen_nested_sums,
};
pub use depth_twenty::{
    ValidatedConstDepthTwentyNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_twenty_nested_sums,
};
pub use depth_twenty_one::{
    ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_twenty_one_nested_sums,
};
pub use depth_twenty_two::{
    ValidatedConstDepthTwentyTwoNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_twenty_two_nested_sums,
};
pub use depth_twenty_three::{
    ValidatedConstDepthTwentyThreeNestedSumOccurrenceMaterialization,
    ValidatedConstRecordWithDepthTwentyThreeNestedSumsMaterialization,
    validate_const_materializable_record_with_depth_twenty_three_nested_sums,
};
use derivation::*;
use report_identity::*;
pub(super) use sum_reachability::SumReachability;
use sum_reachability::{record_sum_profile, reject_sum_array_type};

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-thirteen path set.
///
/// The nested carrier retains its complete plural depth-twelve custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization,
}

impl ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-twelve carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthThirteenRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthThirteenRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-twelve carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_thirteen_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_thirteen_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-thirteen occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_twelve_paths,
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
                    "ConstMaterializable plural depth-thirteen inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_thirteen_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-thirteen fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-thirteen copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_thirteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_thirteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_thirteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_thirteen_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_thirteen_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization {
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

struct DerivedDepthThirteenNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-fourteen path set.
///
/// The nested carrier retains its complete plural depth-thirteen custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization,
}

impl ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-thirteen carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthFourteenRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthFourteenRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-thirteen carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_fourteen_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_fourteen_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-fourteen occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_thirteen_paths,
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
                    "ConstMaterializable plural depth-fourteen inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_fourteen_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-fourteen fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-fourteen copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_fourteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_fourteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_fourteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_fourteen_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_fourteen_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization {
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

struct DerivedDepthFourteenNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-fifteen path set.
///
/// The nested carrier retains its complete plural depth-fourteen custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization,
}

impl ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-fourteen carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthFifteenRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthFifteenRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-fourteen carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_fifteen_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_fifteen_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-fifteen occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_fourteen_paths,
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
                    "ConstMaterializable plural depth-fifteen inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_fifteen_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-fifteen fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-fifteen copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_fifteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_fifteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_fifteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_fifteen_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_fifteen_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization {
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

struct DerivedDepthFifteenNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-twelve path set.
///
/// The nested carrier retains its complete plural depth-eleven custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthElevenNestedSumsMaterialization,
}

impl ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthElevenNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-eleven carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTwelveRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTwelveRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-eleven carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_twelve_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_twelve_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-twelve occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_eleven_paths,
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
                    "ConstMaterializable plural depth-twelve inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_twelve_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-twelve fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-twelve copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_twelve_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_twelve_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_twelve_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_twelve_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_twelve_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization {
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

struct DerivedDepthTwelveNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}
/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-eleven path set.
///
/// The nested carrier retains its complete plural depth-ten custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthElevenNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthTenNestedSumsMaterialization,
}

impl ValidatedConstDepthElevenNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthTenNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-ten carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthElevenNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthElevenRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthElevenNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthElevenNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthElevenRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthElevenNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-ten carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_eleven_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_eleven_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-eleven occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_ten_paths,
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
                    "ConstMaterializable plural depth-eleven inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_eleven_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-eleven fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-eleven copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_eleven_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthElevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_eleven_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_eleven_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthElevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_eleven_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_eleven_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthElevenNestedSumsMaterialization {
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

struct DerivedDepthElevenNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthElevenNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-ten path set.
///
/// The nested carrier retains its complete plural depth-nine custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthTenNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthNineNestedSumsMaterialization,
}

impl ValidatedConstDepthTenNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthNineNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-nine carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTenNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTenRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTenNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTenNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTenRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthTenNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-nine carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_ten_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_depth_ten_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-ten occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_nine_paths,
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
                    "ConstMaterializable plural depth-ten inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_ten_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-ten fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-ten copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_ten_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTenNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_ten_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_ten_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthTenNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_depth_ten_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_ten_nested_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthTenNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthTenNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTenNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-nine path set.
///
/// The nested carrier retains its complete plural depth-eight custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthNineNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthEightNestedSumsMaterialization,
}

impl ValidatedConstDepthNineNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthEightNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-eight carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthNineNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthNineRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthNineNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthNineNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthNineRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthNineNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-eight carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_nine_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_depth_nine_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-nine occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_eight_paths,
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
                    "ConstMaterializable plural depth-nine inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_nine_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-nine fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-nine copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_nine_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthNineNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_nine_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_nine_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthNineNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_depth_nine_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_nine_nested_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthNineNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthNineNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthNineNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-eight path set.
///
/// The nested carrier retains its complete plural depth-seven custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthEightNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthSevenNestedSumsMaterialization,
}

impl ValidatedConstDepthEightNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthSevenNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-seven carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthEightNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthEightRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthEightNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthEightNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthEightRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthEightNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-seven carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_eight_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_eight_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-eight occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_seven_paths,
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
                    "ConstMaterializable plural depth-eight inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_eight_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-eight fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-eight copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_eight_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthEightNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_eight_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_eight_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthEightNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_eight_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_eight_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthEightNestedSumsMaterialization {
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

struct DerivedDepthEightNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthEightNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-seven path set.
///
/// The nested carrier retains its complete plural depth-six custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthSevenNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthSixNestedSumsMaterialization,
}

impl ValidatedConstDepthSevenNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthSixNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-six carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthSevenNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthSevenRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthSevenNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthSevenNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthSevenRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthSevenNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-six carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_seven_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_seven_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-seven occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_six_paths,
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
                    "ConstMaterializable plural depth-seven inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_seven_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-seven fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-seven copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_seven_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthSevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_seven_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_seven_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthSevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_seven_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_seven_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthSevenNestedSumsMaterialization {
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

struct DerivedDepthSevenNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthSevenNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-six path set.
///
/// The nested carrier retains its complete plural depth-five custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthSixNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthFiveNestedSumsMaterialization,
}

impl ValidatedConstDepthSixNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthFiveNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-five carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthSixNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthSixRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthSixNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthSixNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthSixRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthSixNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-five carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-six invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_six_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-six layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_depth_six_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-six custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-six occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_five_paths,
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
                    "ConstMaterializable plural depth-six inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-six bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_six_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-six fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-six copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_six_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthSixNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_six_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_six_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthSixNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_depth_six_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_six_nested_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthSixNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthSixNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthSixNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-five path set.
///
/// The nested carrier retains its complete plural depth-four custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthFiveNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthFourNestedSumsMaterialization,
}

impl ValidatedConstDepthFiveNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthFourNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-four carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthFiveNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthFiveRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFiveNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthFiveNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthFiveRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthFiveNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-four carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-five invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_five_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-five layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_depth_five_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-five custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-five occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_four_paths,
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
                    "ConstMaterializable plural depth-five inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-five bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_five_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-five fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-five copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_five_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFiveNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_five_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_five_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFiveNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_depth_five_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_five_nested_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthFiveNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthFiveNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFiveNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-four path set.
///
/// The nested carrier retains its complete plural depth-three custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthFourNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthThreeNestedSumsMaterialization,
}

impl ValidatedConstDepthFourNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthThreeNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-three carrier. This type
/// deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthFourNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthFourRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFourNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthFourNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthFourRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthFourNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-three carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-four invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_four_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-four layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_depth_four_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-four custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-four occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_three_paths,
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
                    "ConstMaterializable plural depth-four inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-four bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_four_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-four fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-four copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_four_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFourNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_four_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_four_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFourNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_depth_four_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_four_nested_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthFourNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthFourNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthFourNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-three path set.
///
/// The nested carrier retains its own complete authored-order depth-two path
/// set. This type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthThreeNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthTwoNestedSumsMaterialization,
}

impl ValidatedConstDepthThreeNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthTwoNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one existing plural depth-two carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthThreeNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthThreeRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthThreeNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthThreeNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthThreeRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthThreeNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-two carrier before accepting the staged outer image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-three invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_three_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-three layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_three_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-three custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-three occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.depth_two_paths,
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
                    "ConstMaterializable plural depth-three inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-three bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_three_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-three fingerprint drifted after exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay the complete retained custody before one atomic outer-image copy.
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
                "ConstMaterializable plural depth-three copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_three_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthThreeNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_three_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_three_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthThreeNestedSumsMaterialization, MaterializationDiagnostic>
{
    let derived = derive_depth_three_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_three_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthThreeNestedSumsMaterialization {
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

struct DerivedDepthThreeNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthThreeNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one fixed-depth
/// `Outer -> First -> Middle -> Leaf -> direct sums` chain.
///
/// The complete existing depth-two carrier stays nested rather than flattening
/// any child layout or selected sum into the new outer record. This type
/// deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthThreeNestedSumMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthThreeRecordSumPathLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    inner: ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthThreeNestedSumMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthThreeRecordSumPathLayoutReport {
        &self.path_layout
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
        &self.inner
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThreeRecordSumPathLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-three record path invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_three_path_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-three record path layout drifted from retained custody"
                    .into(),
            ));
        }
        let replayed = derive_depth_three_nested_sum_bytes(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
        )?;
        self.inner.replay_against(
            typed,
            replayed.inner.schema_name(),
            &path_layout.depth_two_path,
            replayed.inner.value(),
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
            || replayed
                .inner
                .non_authoritative_materialization_report_fingerprint()
                != self
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-three record path custody drifted from exact replay"
                    .into(),
            ));
        }
        let fingerprint = depth_three_nested_sum_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.inner,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-three record path fingerprint drifted after exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

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
                "ConstMaterializable depth-three record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_three_nested_sum(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthThreeNestedSumMaterialization, MaterializationDiagnostic> {
    let derived =
        derive_depth_three_nested_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_three_nested_sum_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.inner,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthThreeNestedSumMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        inner: derived.inner,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthThreeNestedSumMaterialization {
    schema_report_fingerprint: u64,
    inner: ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    bytes: Vec<u8>,
}

/// Exact custody for one fixed-depth
/// `Outer -> Middle -> Leaf -> direct sums` chain.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTwoRecordSumPathLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    middle: ValidatedConstRecordWithNestedSumRecordMaterialization,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTwoRecordSumPathLayoutReport {
        &self.path_layout
    }

    pub const fn middle(&self) -> &ValidatedConstRecordWithNestedSumRecordMaterialization {
        &self.middle
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwoRecordSumPathLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_two_path_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path layout drifted from retained custody"
                    .into(),
            ));
        }
        let replayed =
            derive_depth_two_nested_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
        self.middle.replay_against(
            typed,
            replayed.middle.schema_name(),
            &path_layout.middle_path,
            replayed.middle.value(),
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
            || replayed
                .middle
                .non_authoritative_materialization_report_fingerprint()
                != self
                    .middle
                    .non_authoritative_materialization_report_fingerprint()
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path custody drifted from exact replay"
                    .into(),
            ));
        }
        let fingerprint = depth_two_nested_sum_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.middle,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path fingerprint drifted after exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

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
                "ConstMaterializable depth-two record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_two_nested_sum(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTwoNestedSumMaterialization, MaterializationDiagnostic> {
    let derived =
        derive_depth_two_nested_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_two_nested_sum_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.middle,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        middle: derived.middle,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthTwoNestedSumMaterialization {
    schema_report_fingerprint: u64,
    middle: ValidatedConstRecordWithNestedSumRecordMaterialization,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-two path set.
///
/// The middle carrier retains its own complete authored-order leaf-record
/// occurrence set. This type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthTwoNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    middle: ValidatedConstRecordWithNestedSumRecordsMaterialization,
}

impl ValidatedConstDepthTwoNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn middle(&self) -> &ValidatedConstRecordWithNestedSumRecordsMaterialization {
        &self.middle
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one existing plural one-level carrier, preserving repeated
/// occurrences without forming a cross-product of layouts or selected values.
/// This type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTwoNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTwoRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwoNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTwoNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTwoRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthTwoNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained middle carrier before accepting the staged outer image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_two_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_depth_two_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-two occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.middle.replay_against_with_reachability(
                typed,
                replayed.middle.schema_name(),
                &path.middle_paths,
                replayed.middle.value(),
                byte_order,
                reachability,
            )?;
            if retained
                .middle
                .non_authoritative_materialization_report_fingerprint()
                != replayed
                    .middle
                    .non_authoritative_materialization_report_fingerprint()
            {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable plural depth-two middle custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_two_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-two fingerprint drifted after exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay the complete retained custody before one atomic outer-image copy.
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
                "ConstMaterializable plural depth-two copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_two_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTwoNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_two_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_two_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthTwoNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_depth_two_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_two_nested_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthTwoNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthTwoNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwoNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one bounded outer-record -> inner-record -> direct-sums
/// materialization path.
///
/// The inner carrier is retained whole rather than flattening selected sums
/// into the outer record. This type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithNestedSumRecordMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalNestedRecordSumPathLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    inner: ValidatedConstRecordWithSumMaterialization,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithNestedSumRecordMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalNestedRecordSumPathLayoutReport {
        &self.path_layout
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithSumMaterialization {
        &self.inner
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the exact outer field and independently reconstruct both
    /// record layers and every selected child sum.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalNestedRecordSumPathLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !nested_path_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed =
            derive_nested_record_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
        self.inner.replay_against_sum_fields(
            typed,
            replayed.inner.schema_name(),
            &path_layout.inner_layout,
            &path_layout.child_sum_layouts,
            replayed.inner.value(),
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
            || replayed
                .inner
                .non_authoritative_materialization_report_fingerprint()
                != self
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path custody drifted from exact replay".into(),
            ));
        }
        let fingerprint = nested_record_sum_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.inner,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path report fingerprint drifted from exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay every retained fact before one atomic copy of the outer image.
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
                "ConstMaterializable nested-record path copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Validate the singular one-level record path and retain the inner direct-sum
/// carrier as one atomic outer field value.
pub fn validate_const_materializable_record_with_nested_sum_record(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalNestedRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithNestedSumRecordMaterialization, MaterializationDiagnostic> {
    let derived =
        derive_nested_record_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = nested_record_sum_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.inner,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithNestedSumRecordMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        inner: derived.inner,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

/// Exact custody for the complete authored-order set of qualifying direct
/// inner-record occurrences. Each occurrence retains one independent existing
/// direct-sum record carrier; the outer layout is retained only once.
#[derive(Debug)]
pub struct ValidatedConstRecordWithNestedSumRecordsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalNestedRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    inner_records: Vec<ValidatedConstNestedSumRecordOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithNestedSumRecordsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalNestedRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn inner_records(&self) -> &[ValidatedConstNestedSumRecordOccurrenceMaterialization] {
        &self.inner_records
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
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

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !nested_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths layout drifted from retained custody"
                    .into(),
            ));
        }
        let replayed = derive_nested_record_sums_bytes_with_reachability(
            typed,
            schema_name,
            NestedPathsView::Plural(path_layout),
            value,
            byte_order,
            reachability,
        )?;
        if replayed.inner_records.len() != self.inner_records.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record occurrence custody changed cardinality".into(),
            ));
        }
        for (((retained, replayed), path), retained_path) in self
            .inner_records
            .iter()
            .zip(&replayed.inner_records)
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
                    "ConstMaterializable nested-record occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_sum_fields(
                typed,
                replayed.inner.schema_name(),
                &path.inner_layout,
                &path.child_sum_layouts,
                replayed.inner.value(),
                byte_order,
            )?;
            if retained
                .inner
                .non_authoritative_materialization_report_fingerprint()
                != replayed
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
            {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable nested-record inner report coordinate drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths bytes drifted from exact replay".into(),
            ));
        }
        let fingerprint = nested_record_sums_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.inner_records,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths report fingerprint drifted after exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

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
                "ConstMaterializable nested-record paths copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_nested_sum_records(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithNestedSumRecordsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_nested_sum_records_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_nested_sum_records_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithNestedSumRecordsMaterialization, MaterializationDiagnostic> {
    let derived = derive_nested_record_sums_bytes_with_reachability(
        typed,
        schema_name,
        NestedPathsView::Plural(path_layout),
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = nested_record_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.inner_records,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithNestedSumRecordsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        inner_records: derived.inner_records,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedNestedRecordSumMaterialization {
    schema_report_fingerprint: u64,
    inner: ValidatedConstRecordWithSumMaterialization,
    bytes: Vec<u8>,
}

struct DerivedNestedRecordSumsMaterialization {
    schema_report_fingerprint: u64,
    inner_records: Vec<ValidatedConstNestedSumRecordOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct ValidatedConstNestedSumRecordOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithSumMaterialization,
}

impl ValidatedConstNestedSumRecordOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithSumMaterialization {
        &self.inner
    }
}

#[derive(Clone, Copy)]
struct NestedPathOccurrenceView<'a> {
    outer_field: &'a str,
    outer_member_identity: Option<u64>,
    inner_layout: &'a psi_layout_plans::LayoutPlanReport,
    child_sum_layouts: &'a [psi_layout_plans::ConventionalSumFieldLayoutReport],
}

#[derive(Clone, Copy)]
enum NestedPathsView<'a> {
    Singular(&'a ConventionalNestedRecordSumPathLayoutReport),
    Plural(&'a ConventionalNestedRecordSumPathsLayoutReport),
}

impl<'a> NestedPathsView<'a> {
    fn outer_layout(self) -> &'a psi_layout_plans::LayoutPlanReport {
        match self {
            Self::Singular(report) => &report.outer_layout,
            Self::Plural(report) => &report.outer_layout,
        }
    }

    fn len(self) -> usize {
        match self {
            Self::Singular(_) => 1,
            Self::Plural(report) => report.paths.len(),
        }
    }

    fn get(self, index: usize) -> Option<NestedPathOccurrenceView<'a>> {
        match self {
            Self::Singular(report) if index == 0 => Some(NestedPathOccurrenceView {
                outer_field: &report.outer_field,
                outer_member_identity: report.outer_member_identity,
                inner_layout: &report.inner_layout,
                child_sum_layouts: &report.child_sum_layouts,
            }),
            Self::Singular(_) => None,
            Self::Plural(report) => report
                .paths
                .get(index)
                .map(|path| NestedPathOccurrenceView {
                    outer_field: &path.outer_field,
                    outer_member_identity: path.outer_member_identity,
                    inner_layout: &path.inner_layout,
                    child_sum_layouts: &path.child_sum_layouts,
                }),
        }
    }
}
