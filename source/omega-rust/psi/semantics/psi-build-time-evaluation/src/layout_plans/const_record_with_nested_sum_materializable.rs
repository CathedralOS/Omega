//! Value-sensitive materialization of record fields containing records with
//! direct conventional pure-sum fields.

use psi_layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder,
    ConventionalDepthEightRecordSumPathsLayoutReport,
    ConventionalDepthElevenRecordSumPathsLayoutReport,
    ConventionalDepthFiveRecordSumPathsLayoutReport,
    ConventionalDepthFourRecordSumPathsLayoutReport,
    ConventionalDepthNineRecordSumPathsLayoutReport,
    ConventionalDepthSevenRecordSumPathsLayoutReport,
    ConventionalDepthSixRecordSumPathsLayoutReport, ConventionalDepthTenRecordSumPathsLayoutReport,
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

fn derive_depth_two_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTwoNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-two outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-two occurrence set exceeds compiler resources".into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-two path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                let profile = record_sum_profile(typed, named, reachability)?;
                if profile.direct {
                    return Err(MaterializationDiagnostic(format!(
                        "field `{}` reaches a sum one record layer too early for the plural depth-two rung",
                        field.name
                    )));
                }
                if profile.array {
                    return Err(MaterializationDiagnostic(format!(
                        "field `{}` reaches sums through an array, outside the plural depth-two rung",
                        field.name
                    )));
                }
                if profile.deeper {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-two paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-two report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        total_leaf_occurrences = total_leaf_occurrences
            .checked_add(path.middle_paths.paths.len())
            .ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable plural depth-two leaf occurrence count overflows".into(),
                )
            })?;
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-two middle custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (middle_field, middle_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            middle_field.name.as_str(),
            middle_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-two path for `{}` is missing, duplicated, or out of authored field order",
                middle_field.name
            )));
        }
        let middle_value = supplied
            .get(middle_field.name.as_str())
            .expect("complete outer value checked above");
        let middle =
            validate_const_materializable_record_with_nested_sum_records_with_reachability(
                typed,
                middle_data.name.as_str(),
                &path.middle_paths,
                middle_value,
                byte_order,
                reachability,
            )?;
        let middle_size = path.middle_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-two path `{}` requires one exact middle extent",
                middle_field.name
            ))
        })?;
        if usize::try_from(middle_size).ok() != Some(middle.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-two middle bytes for `{}` do not cover the exact middle extent",
                middle_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthTwoNestedSumOccurrenceMaterialization {
            outer_field: middle_field.name.to_string(),
            outer_member_identity: middle_field.identity,
            middle,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-two outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-two active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut middle_bytes = Vec::new();
            middle_bytes
                .try_reserve_exact(occurrence.middle.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-two middle staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            middle_bytes.extend_from_slice(occurrence.middle.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .middle_paths
                    .outer_layout
                    .size
                    .expect("validated middle extent"),
                align: path.middle_paths.outer_layout.align,
                repeated: None,
                bytes: middle_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-two staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-two outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-two outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-two staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-two schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-two value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthTwoNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_twelve_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTwelveNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-twelve outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-twelve occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-twelve path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-twelve paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-twelve report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for eleventh_occurrence in &path.depth_eleven_paths.paths {
            for tenth_occurrence in &eleventh_occurrence.depth_ten_paths.paths {
                for ninth_occurrence in &tenth_occurrence.depth_nine_paths.paths {
                    for eighth_occurrence in &ninth_occurrence.depth_eight_paths.paths {
                        for seventh_occurrence in &eighth_occurrence.depth_seven_paths.paths {
                            for sixth_occurrence in &seventh_occurrence.depth_six_paths.paths {
                                for fifth_occurrence in &sixth_occurrence.depth_five_paths.paths {
                                    for fourth_occurrence in
                                        &fifth_occurrence.depth_four_paths.paths
                                    {
                                        for third_occurrence in
                                            &fourth_occurrence.depth_three_paths.paths
                                        {
                                            for second_occurrence in
                                                &third_occurrence.depth_two_paths.paths
                                            {
                                                total_leaf_occurrences = total_leaf_occurrences
                                                    .checked_add(
                                                        second_occurrence
                                                            .middle_paths
                                                            .paths
                                                            .len(),
                                                    )
                                                    .ok_or_else(|| {
                                                        MaterializationDiagnostic(
                                                            "ConstMaterializable plural depth-twelve leaf occurrence count overflows"
                                                                .into(),
                                                        )
                                                    })?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-twelve path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_eleven_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_eleven_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_eleven_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-twelve path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-twelve inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(
            ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization {
                outer_field: inner_field.name.to_string(),
                outer_member_identity: inner_field.identity,
                inner,
            },
        );
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-twelve active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-twelve inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_eleven_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-eleven extent"),
                align: path.depth_eleven_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-twelve staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-twelve outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-twelve outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-twelve staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-twelve value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthTwelveNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_eleven_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthElevenNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-eleven outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eleven occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-eleven path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-eleven paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-eleven report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for tenth_occurrence in &path.depth_ten_paths.paths {
            for ninth_occurrence in &tenth_occurrence.depth_nine_paths.paths {
                for eighth_occurrence in &ninth_occurrence.depth_eight_paths.paths {
                    for seventh_occurrence in &eighth_occurrence.depth_seven_paths.paths {
                        for sixth_occurrence in &seventh_occurrence.depth_six_paths.paths {
                            for fifth_occurrence in &sixth_occurrence.depth_five_paths.paths {
                                for fourth_occurrence in &fifth_occurrence.depth_four_paths.paths {
                                    for third_occurrence in
                                        &fourth_occurrence.depth_three_paths.paths
                                    {
                                        for second_occurrence in
                                            &third_occurrence.depth_two_paths.paths
                                        {
                                            total_leaf_occurrences = total_leaf_occurrences
                                        .checked_add(second_occurrence.middle_paths.paths.len())
                                        .ok_or_else(|| {
                                            MaterializationDiagnostic(
                                                "ConstMaterializable plural depth-eleven leaf occurrence count overflows"
                                                    .into(),
                                            )
                                        })?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-eleven path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_ten_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_ten_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_ten_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-eleven path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-eleven inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(
            ValidatedConstDepthElevenNestedSumOccurrenceMaterialization {
                outer_field: inner_field.name.to_string(),
                outer_member_identity: inner_field.identity,
                inner,
            },
        );
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eleven active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-eleven inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_ten_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-ten extent"),
                align: path.depth_ten_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-eleven staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-eleven outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eleven outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eleven staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eleven value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthElevenNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_ten_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTenNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-ten outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-ten occurrence set exceeds compiler resources".into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-ten path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-ten paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-ten report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for ninth_occurrence in &path.depth_nine_paths.paths {
            for eighth_occurrence in &ninth_occurrence.depth_eight_paths.paths {
                for seventh_occurrence in &eighth_occurrence.depth_seven_paths.paths {
                    for sixth_occurrence in &seventh_occurrence.depth_six_paths.paths {
                        for fifth_occurrence in &sixth_occurrence.depth_five_paths.paths {
                            for fourth_occurrence in &fifth_occurrence.depth_four_paths.paths {
                                for third_occurrence in &fourth_occurrence.depth_three_paths.paths {
                                    for second_occurrence in &third_occurrence.depth_two_paths.paths
                                    {
                                        total_leaf_occurrences = total_leaf_occurrences
                                        .checked_add(second_occurrence.middle_paths.paths.len())
                                        .ok_or_else(|| {
                                            MaterializationDiagnostic(
                                                "ConstMaterializable plural depth-ten leaf occurrence count overflows"
                                                    .into(),
                                            )
                                        })?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-ten path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_nine_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_nine_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_nine_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-ten path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-ten inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthTenNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-ten active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-ten inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_nine_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-nine extent"),
                align: path.depth_nine_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-ten staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-ten outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-ten outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-ten staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-ten value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthTenNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_nine_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthNineNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-nine outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-nine occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-nine path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-nine paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-nine report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for eighth_occurrence in &path.depth_eight_paths.paths {
            for seventh_occurrence in &eighth_occurrence.depth_seven_paths.paths {
                for sixth_occurrence in &seventh_occurrence.depth_six_paths.paths {
                    for fifth_occurrence in &sixth_occurrence.depth_five_paths.paths {
                        for fourth_occurrence in &fifth_occurrence.depth_four_paths.paths {
                            for third_occurrence in &fourth_occurrence.depth_three_paths.paths {
                                for second_occurrence in &third_occurrence.depth_two_paths.paths {
                                    total_leaf_occurrences = total_leaf_occurrences
                                        .checked_add(second_occurrence.middle_paths.paths.len())
                                        .ok_or_else(|| {
                                            MaterializationDiagnostic(
                                                "ConstMaterializable plural depth-nine leaf occurrence count overflows"
                                                    .into(),
                                            )
                                        })?;
                                }
                            }
                        }
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-nine path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_eight_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_eight_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_eight_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-nine path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-nine inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthNineNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-nine active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-nine inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_eight_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-eight extent"),
                align: path.depth_eight_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-nine staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-nine outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-nine outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-nine staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-nine value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthNineNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_eight_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthEightNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-eight outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eight occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-eight path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-eight paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-eight report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for seventh_occurrence in &path.depth_seven_paths.paths {
            for sixth_occurrence in &seventh_occurrence.depth_six_paths.paths {
                for fifth_occurrence in &sixth_occurrence.depth_five_paths.paths {
                    for fourth_occurrence in &fifth_occurrence.depth_four_paths.paths {
                        for third_occurrence in &fourth_occurrence.depth_three_paths.paths {
                            for second_occurrence in &third_occurrence.depth_two_paths.paths {
                                total_leaf_occurrences = total_leaf_occurrences
                                    .checked_add(second_occurrence.middle_paths.paths.len())
                                    .ok_or_else(|| {
                                        MaterializationDiagnostic(
                                            "ConstMaterializable plural depth-eight leaf occurrence count overflows"
                                                .into(),
                                        )
                                    })?;
                            }
                        }
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-eight path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_seven_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_seven_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_seven_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-eight path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-eight inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthEightNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eight active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-eight inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_seven_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-seven extent"),
                align: path.depth_seven_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-eight staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-eight outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eight outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-eight staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-eight value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthEightNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_seven_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthSevenNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-seven outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-seven occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-seven path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-seven paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-seven report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for sixth_occurrence in &path.depth_six_paths.paths {
            for fifth_occurrence in &sixth_occurrence.depth_five_paths.paths {
                for fourth_occurrence in &fifth_occurrence.depth_four_paths.paths {
                    for third_occurrence in &fourth_occurrence.depth_three_paths.paths {
                        for second_occurrence in &third_occurrence.depth_two_paths.paths {
                            total_leaf_occurrences = total_leaf_occurrences
                                .checked_add(second_occurrence.middle_paths.paths.len())
                                .ok_or_else(|| {
                                    MaterializationDiagnostic(
                                        "ConstMaterializable plural depth-seven leaf occurrence count overflows"
                                            .into(),
                                    )
                                })?;
                        }
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-seven path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_six_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_six_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_six_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-seven path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-seven inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthSevenNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-seven active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-seven inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_six_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-six extent"),
                align: path.depth_six_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-seven staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-seven outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-seven outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-seven staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-seven value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthSevenNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_six_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthSixNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-six outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-six occurrence set exceeds compiler resources".into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-six path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-six paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-six report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for fourth_occurrence in &path.depth_five_paths.paths {
            for third_occurrence in &fourth_occurrence.depth_four_paths.paths {
                for second_occurrence in &third_occurrence.depth_three_paths.paths {
                    for first_occurrence in &second_occurrence.depth_two_paths.paths {
                        total_leaf_occurrences = total_leaf_occurrences
                            .checked_add(first_occurrence.middle_paths.paths.len())
                            .ok_or_else(|| {
                                MaterializationDiagnostic(
                                    "ConstMaterializable plural depth-six leaf occurrence count overflows"
                                        .into(),
                                )
                            })?;
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-six paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-six inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-six path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_five_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_five_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_five_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-six path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-six inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthSixNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-six outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-six active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-six inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_five_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-five extent"),
                align: path.depth_five_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-six staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-six outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-six outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-six staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-six schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-six value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthSixNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_five_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthFiveNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-five outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-five occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-five path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-five paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-five report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for third_occurrence in &path.depth_four_paths.paths {
            for second_occurrence in &third_occurrence.depth_three_paths.paths {
                for first_occurrence in &second_occurrence.depth_two_paths.paths {
                    total_leaf_occurrences = total_leaf_occurrences
                        .checked_add(first_occurrence.middle_paths.paths.len())
                        .ok_or_else(|| {
                            MaterializationDiagnostic(
                                "ConstMaterializable plural depth-five leaf occurrence count overflows"
                                    .into(),
                            )
                        })?;
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-five paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-five inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-five path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_four_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_four_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_four_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-five path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-five inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthFiveNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-five outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-five active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-five inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_four_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-four extent"),
                align: path.depth_four_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-five staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-five outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-five outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-five staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-five schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-five value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthFiveNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_four_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthFourNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-four outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-four occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-four path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-four paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-four report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for second_occurrence in &path.depth_three_paths.paths {
            for first_occurrence in &second_occurrence.depth_two_paths.paths {
                total_leaf_occurrences = total_leaf_occurrences
                    .checked_add(first_occurrence.middle_paths.paths.len())
                    .ok_or_else(|| {
                        MaterializationDiagnostic(
                            "ConstMaterializable plural depth-four leaf occurrence count overflows"
                                .into(),
                        )
                    })?;
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-four paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-four inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-four path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_three_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_three_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_three_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-four path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-four inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthFourNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-four outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-four active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-four inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_three_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-three extent"),
                align: path.depth_three_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-four staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-four outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-four outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-four staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-four schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-four value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthFourNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_three_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthThreeNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-three outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-three occurrence set exceeds compiler resources"
                .into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable plural depth-three path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                if reachability.type_contains_sum(field.type_reference)? {
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-three paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-three report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for occurrence in &path.depth_two_paths.paths {
            total_leaf_occurrences = total_leaf_occurrences
                .checked_add(occurrence.middle_paths.paths.len())
                .ok_or_else(|| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-three leaf occurrence count overflows"
                            .into(),
                    )
                })?;
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-three paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-three inner custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .paths
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-three path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_two_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.depth_two_paths,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.depth_two_paths.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-three path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-three inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstDepthThreeNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-three outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-three active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        let current_occurrence = occurrences
            .get(occurrence_index)
            .zip(path_layout.paths.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable plural depth-three inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .depth_two_paths
                    .outer_layout
                    .size
                    .expect("validated plural depth-two extent"),
                align: path.depth_two_paths.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let field_bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if u64::try_from(field_bytes.len()).ok() != Some(size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                field_bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes: field_bytes,
        });
    }
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable plural depth-three staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-three outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-three outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-three staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-three schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-three value staging exceeds compiler resources"
                    .into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthThreeNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

fn derive_depth_three_nested_sum_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedDepthThreeNestedSumMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable depth-three outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidate = None;
    let mut reachability = SumReachability::new(typed);
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        if !reachability.type_contains_sum(field.type_reference)? {
            continue;
        }
        if matches!(
            typed
                .type_reference_table
                .type_reference(field.type_reference),
            TypeReferenceNode::FixedArray { .. }
        ) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} reaches a sum through an array, outside the depth-three record path rung",
                field.name
            )));
        }
        let named = exact_named_data(typed, field.type_reference)?.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "value.{} lacks one exact enclosing-record identity",
                field.name
            ))
        })?;
        if DataDefinition::shape_kind_from_members(typed.data_members(named))
            != DataShapeKind::Record
        {
            return Err(MaterializationDiagnostic(format!(
                "value.{} does not name the required enclosing record",
                field.name
            )));
        }
        if candidate.is_some() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-three path requires exactly one sum-reachable outer record field"
                    .into(),
            ));
        }
        candidate = Some((field, named));
    }
    let Some((inner_field, inner_data)) = candidate else {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable depth-three path requires exactly one qualifying record chain"
                .into(),
        ));
    };
    if !field_occurrence_matches(
        &path_layout.outer_field,
        path_layout.outer_member_identity,
        inner_field.name.as_str(),
        inner_field.identity,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable depth-three path does not name exact outer field `{}`",
            inner_field.name
        )));
    }
    let inner_value = supplied
        .get(inner_field.name.as_str())
        .expect("complete outer value checked above");
    let inner = validate_const_materializable_record_with_depth_two_nested_sum(
        typed,
        inner_data.name.as_str(),
        &path_layout.depth_two_path,
        inner_value,
        byte_order,
    )?;
    let inner_size = path_layout
        .depth_two_path
        .outer_layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable depth-three path requires one exact inner extent".into(),
            )
        })?;
    if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable depth-three inner bytes do not cover the exact inner extent"
                .into(),
        ));
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable depth-three outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable depth-three active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        if field_occurrence_matches(
            field.name.as_str(),
            field.identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable depth-three inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: inner_size,
                align: path_layout.depth_two_path.outer_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if bytes.len() as u64 != size {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes,
        });
    }
    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated depth-three outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable depth-three outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable depth-three staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable depth-three schema staging exceeds compiler resources".into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable depth-three value staging exceeds compiler resources".into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthThreeNestedSumMaterialization {
        schema_report_fingerprint,
        inner,
        bytes,
    })
}

fn derive_depth_two_nested_sum_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedDepthTwoNestedSumMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable depth-two outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidate = None;
    let mut reachability = SumReachability::new(typed);
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        if !reachability.type_contains_sum(field.type_reference)? {
            continue;
        }
        if matches!(
            typed
                .type_reference_table
                .type_reference(field.type_reference),
            TypeReferenceNode::FixedArray { .. }
        ) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} reaches a sum through an array, outside the depth-two record path rung",
                field.name
            )));
        }
        let named = exact_named_data(typed, field.type_reference)?.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "value.{} lacks one exact middle-record identity",
                field.name
            ))
        })?;
        if DataDefinition::shape_kind_from_members(typed.data_members(named))
            != DataShapeKind::Record
        {
            return Err(MaterializationDiagnostic(format!(
                "value.{} does not name the required middle record",
                field.name
            )));
        }
        if candidate.is_some() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two path requires exactly one sum-reachable outer record field"
                    .into(),
            ));
        }
        candidate = Some((field, named));
    }
    let Some((middle_field, middle_data)) = candidate else {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable depth-two path requires exactly one qualifying record chain"
                .into(),
        ));
    };
    if !field_occurrence_matches(
        &path_layout.outer_field,
        path_layout.outer_member_identity,
        middle_field.name.as_str(),
        middle_field.identity,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable depth-two path does not name exact outer field `{}`",
            middle_field.name
        )));
    }
    let middle_value = supplied
        .get(middle_field.name.as_str())
        .expect("complete outer value checked above");
    let middle = validate_const_materializable_record_with_nested_sum_record(
        typed,
        middle_data.name.as_str(),
        &path_layout.middle_path,
        middle_value,
        byte_order,
    )?;
    let middle_size = path_layout.middle_path.outer_layout.size.ok_or_else(|| {
        MaterializationDiagnostic(
            "ConstMaterializable depth-two path requires one exact middle extent".into(),
        )
    })?;
    if usize::try_from(middle_size).ok() != Some(middle.bytes().len()) {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable depth-two middle bytes do not cover the exact middle extent"
                .into(),
        ));
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable depth-two outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = vec![data.symbol];
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        if field_occurrence_matches(
            field.name.as_str(),
            field.identity,
            middle_field.name.as_str(),
            middle_field.identity,
        ) {
            let mut middle_bytes = Vec::new();
            middle_bytes
                .try_reserve_exact(middle.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable depth-two middle staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            middle_bytes.extend_from_slice(middle.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: middle_size,
                align: path_layout.middle_path.outer_layout.align,
                repeated: None,
                bytes: middle_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if bytes.len() as u64 != size {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes,
        });
    }
    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated depth-two outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable depth-two outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable depth-two staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable depth-two schema staging exceeds compiler resources".into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable depth-two value staging exceeds compiler resources".into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(&path_layout.outer_layout, &schemas, &values, &mut bytes)?;
    Ok(DerivedDepthTwoNestedSumMaterialization {
        schema_report_fingerprint,
        middle,
        bytes,
    })
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

fn derive_nested_record_sum_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalNestedRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedNestedRecordSumMaterialization, MaterializationDiagnostic> {
    let mut derived = derive_nested_record_sums_bytes(
        typed,
        schema_name,
        NestedPathsView::Singular(path_layout),
        value,
        byte_order,
    )?;
    if derived.inner_records.len() != 1 {
        return Err(MaterializationDiagnostic(format!(
            "singular ConstMaterializable nested-record path requires exactly one qualifying occurrence; found {}",
            derived.inner_records.len()
        )));
    }
    let occurrence = derived.inner_records.pop().expect("exactly one occurrence");
    Ok(DerivedNestedRecordSumMaterialization {
        schema_report_fingerprint: derived.schema_report_fingerprint,
        inner: occurrence.inner,
        bytes: derived.bytes,
    })
}

fn derive_nested_record_sums_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: NestedPathsView<'_>,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedNestedRecordSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    derive_nested_record_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn derive_nested_record_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: NestedPathsView<'_>,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedNestedRecordSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout().schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-record outer layout schema report fingerprint does not match `{schema_name}`"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected record `{schema_name}`, found {}",
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "value record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(schema_name, fields)?;
    let members = typed.data_members(data);
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "value supplies {} field(s), expected {} for `{schema_name}`",
            supplied.len(),
            members.len()
        )));
    }

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable nested-record occurrence set exceeds compiler resources".into(),
        )
    })?;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "value has no declared field `{}`",
                field.name
            )));
        }
        if field.relevance.is_erased() {
            continue;
        }
        reject_sum_array_type(
            typed,
            field.type_reference,
            &format!("value.{}", field.name),
            reachability,
        )?;
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable nested-record path does not admit direct outer sum field `{}`",
                    field.name
                )));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape",
                    field.name
                )));
            }
            DataShapeKind::Record => {
                let profile = record_sum_profile(typed, named, reachability)?;
                if profile.direct {
                    if profile.array || profile.deeper {
                        return Err(MaterializationDiagnostic(format!(
                            "inner record field `{}` combines direct sums with an array or deeper sum path",
                            field.name
                        )));
                    }
                    validate_outer_record_owner(typed, named)?;
                    candidates.push((field, named));
                } else if profile.array || profile.deeper {
                    return Err(MaterializationDiagnostic(format!(
                        "field `{}` reaches sums beyond the admitted direct child path",
                        field.name
                    )));
                }
            }
            DataShapeKind::Empty => {}
        }
    }
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable nested-record paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-record path report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.len(),
            candidates.len()
        )));
    }
    let mut inner_records = Vec::new();
    inner_records
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record inner custody exceeds compiler resources".into(),
            )
        })?;
    for (index, (inner_field, inner_data)) in candidates.iter().enumerate() {
        let path = path_layout
            .get(index)
            .expect("path cardinality checked above");
        if !field_occurrence_matches(
            path.outer_field,
            path.outer_member_identity,
            inner_field.name.as_str(),
            inner_field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-record path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete record value checked above");
        let inner = validate_const_materializable_record_with_conventional_sums(
            typed,
            inner_data.name.as_str(),
            path.inner_layout,
            path.child_sum_layouts,
            inner_value,
            byte_order,
        )?;
        let inner_size = path.inner_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable nested-record path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-record inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        inner_records.push(ValidatedConstNestedSumRecordOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = vec![data.symbol];
    let mut occurrence_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete record value checked above");
        let current_occurrence = inner_records
            .get(occurrence_index)
            .zip(path_layout.get(occurrence_index));
        if let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
            field_occurrence_matches(
                occurrence.outer_field(),
                occurrence.outer_member_identity(),
                field.name.as_str(),
                field.identity,
            )
        }) {
            occurrence_index += 1;
            let mut inner_bytes = Vec::new();
            inner_bytes
                .try_reserve_exact(occurrence.inner.bytes().len())
                .map_err(|_| {
                    MaterializationDiagnostic(
                        "ConstMaterializable nested-record inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path.inner_layout.size.expect("validated inner extent"),
                align: path.inner_layout.align,
                repeated: None,
                bytes: inner_bytes,
            });
            continue;
        }
        validate_value(
            typed,
            field.type_reference,
            field_value,
            &format!("value.{}", field.name),
            &mut active,
        )?;
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, _, _, _, _, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value.{} is outside the target-independent fixed aggregate subset",
                    field.name
                ))
            })?;
        let bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut active,
        )?;
        if bytes.len() as u64 != size {
            return Err(MaterializationDiagnostic(format!(
                "value.{} encoded to {} bytes, expected {size}",
                field.name,
                bytes.len()
            )));
        }
        encoded_fields.push(EncodedOuterField {
            name: field.name.to_string(),
            identity: field.identity,
            size,
            align,
            repeated,
            bytes,
        });
    }
    if occurrence_index != inner_records.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable nested-record occurrence staging did not consume the complete authored-order set"
                .into(),
        ));
    }
    validate_outer_layout(path_layout.outer_layout(), &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout()
            .size
            .expect("validated outer fixed extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable nested-record outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable nested-record staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record value staging exceeds compiler resources".into(),
            )
        })?;
    for field in encoded_fields {
        let schema = match (field.repeated, field.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &field.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &field.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&field.name, identity, field.size)?
            }
            (None, None) => AggregateFieldSchema::new(&field.name, field.size)?,
        };
        schemas.push(schema);
        values.push(AggregateFieldValue::new(field.name, field.bytes)?);
    }
    materialize_aggregate_layout_into(path_layout.outer_layout(), &schemas, &values, &mut bytes)?;
    Ok(DerivedNestedRecordSumsMaterialization {
        schema_report_fingerprint,
        inner_records,
        bytes,
    })
}

#[derive(Default)]
struct RecordSumProfile {
    direct: bool,
    array: bool,
    deeper: bool,
}

fn record_sum_profile(
    typed: &TypedTrees,
    data: &DataDefinition,
    reachability: &mut SumReachability<'_>,
) -> Result<RecordSumProfile, MaterializationDiagnostic> {
    let mut profile = RecordSumProfile::default();
    for member in typed.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        match typed
            .type_reference_table
            .type_reference(field.type_reference)
        {
            TypeReferenceNode::Named { .. } => {
                let Some(named) = exact_named_data(typed, field.type_reference)? else {
                    continue;
                };
                match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
                    DataShapeKind::Enum => profile.direct = true,
                    DataShapeKind::Record => {
                        if reachability.type_contains_sum(field.type_reference)? {
                            profile.deeper = true;
                        }
                    }
                    DataShapeKind::Mixed => {
                        return Err(MaterializationDiagnostic(format!(
                            "field `{}` uses a mixed common-field/case shape",
                            field.name
                        )));
                    }
                    DataShapeKind::Empty => {}
                }
            }
            TypeReferenceNode::FixedArray { .. } => {
                if reachability.type_contains_sum(field.type_reference)? {
                    profile.array = true;
                }
            }
            _ => {}
        }
    }
    Ok(profile)
}

fn reject_sum_array_type(
    typed: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
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

#[derive(Clone, Copy)]
enum ReachabilityState {
    Visiting,
    Done(bool),
}

struct ReachabilityFrame<'a> {
    data: &'a DataDefinition,
    next_member: usize,
    found: bool,
}

pub(super) struct SumReachability<'a> {
    typed: &'a TypedTrees,
    states: std::collections::HashMap<(u32, u32), ReachabilityState>,
    traversed_edges: usize,
}

impl<'a> SumReachability<'a> {
    const MAX_RECORDS: usize = 4096;
    const MAX_EDGES: usize = 16384;

    pub(super) fn new(typed: &'a TypedTrees) -> Self {
        Self {
            typed,
            states: std::collections::HashMap::new(),
            traversed_edges: 0,
        }
    }

    pub(super) fn type_contains_sum(
        &mut self,
        mut type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Result<bool, MaterializationDiagnostic> {
        let mut array_depth = 0usize;
        while let TypeReferenceNode::FixedArray { element_type, .. } = self
            .typed
            .type_reference_table
            .type_reference(type_reference)
        {
            array_depth += 1;
            if array_depth > 64 {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable nested-record path exceeds bounded fixed-array depth"
                        .into(),
                ));
            }
            type_reference = *element_type;
        }
        let Some(data) = exact_named_data(self.typed, type_reference)? else {
            return Ok(false);
        };
        match DataDefinition::shape_kind_from_members(self.typed.data_members(data)) {
            DataShapeKind::Enum | DataShapeKind::Mixed => Ok(true),
            DataShapeKind::Empty => Ok(false),
            DataShapeKind::Record => self.record_contains_sum(data),
        }
    }

    fn record_contains_sum(
        &mut self,
        root: &'a DataDefinition,
    ) -> Result<bool, MaterializationDiagnostic> {
        let root_identity = symbol_identity(root.symbol)?;
        if let Some(state) = self.states.get(&root_identity) {
            return match state {
                ReachabilityState::Done(found) => Ok(*found),
                ReachabilityState::Visiting => Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable nested-record path is recursive through `{}`",
                    root.name
                ))),
            };
        }
        self.insert_state(root_identity, ReachabilityState::Visiting)?;
        let mut stack = Vec::new();
        stack.try_reserve(1).map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record traversal stack exceeds compiler resources"
                    .into(),
            )
        })?;
        stack.push(ReachabilityFrame {
            data: root,
            next_member: 0,
            found: false,
        });

        loop {
            let Some(frame) = stack.last_mut() else {
                unreachable!("root reachability frame returns when completed")
            };
            let members = self.typed.data_members(frame.data);
            // `found` is the eventual answer, not permission to skip later
            // fields: a later branch can still expose a cycle, malformed
            // nominal identity, or resource-bound failure.
            if frame.next_member == members.len() {
                let completed = stack.pop().expect("active reachability frame");
                let identity = symbol_identity(completed.data.symbol)?;
                self.states
                    .insert(identity, ReachabilityState::Done(completed.found));
                if let Some(parent) = stack.last_mut() {
                    parent.found |= completed.found;
                    continue;
                }
                return Ok(completed.found);
            }
            let member = &members[frame.next_member];
            frame.next_member += 1;
            let DataMember::Field(field) = member else {
                frame.found = true;
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            self.traversed_edges = self.traversed_edges.checked_add(1).ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable nested-record traversal edge count overflows".into(),
                )
            })?;
            if self.traversed_edges > Self::MAX_EDGES {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable nested-record path exceeds bounded schema traversal edges"
                        .into(),
                ));
            }
            let mut child_type = field.type_reference;
            let mut array_depth = 0usize;
            while let TypeReferenceNode::FixedArray { element_type, .. } =
                self.typed.type_reference_table.type_reference(child_type)
            {
                array_depth += 1;
                if array_depth > 64 {
                    return Err(MaterializationDiagnostic(
                        "ConstMaterializable nested-record path exceeds bounded fixed-array depth"
                            .into(),
                    ));
                }
                child_type = *element_type;
            }
            let Some(child) = exact_named_data(self.typed, child_type)? else {
                continue;
            };
            match DataDefinition::shape_kind_from_members(self.typed.data_members(child)) {
                DataShapeKind::Enum | DataShapeKind::Mixed => frame.found = true,
                DataShapeKind::Empty => {}
                DataShapeKind::Record => {
                    let identity = symbol_identity(child.symbol)?;
                    match self.states.get(&identity).copied() {
                        Some(ReachabilityState::Done(found)) => frame.found |= found,
                        Some(ReachabilityState::Visiting) => {
                            return Err(MaterializationDiagnostic(format!(
                                "ConstMaterializable nested-record path is recursive through `{}`",
                                child.name
                            )));
                        }
                        None => {
                            self.insert_state(identity, ReachabilityState::Visiting)?;
                            stack.try_reserve(1).map_err(|_| {
                                MaterializationDiagnostic(
                                    "ConstMaterializable nested-record traversal stack exceeds compiler resources"
                                        .into(),
                                )
                            })?;
                            stack.push(ReachabilityFrame {
                                data: child,
                                next_member: 0,
                                found: false,
                            });
                        }
                    }
                }
            }
        }
    }

    fn insert_state(
        &mut self,
        identity: (u32, u32),
        state: ReachabilityState,
    ) -> Result<(), MaterializationDiagnostic> {
        if self.states.len() >= Self::MAX_RECORDS {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path exceeds bounded schema traversal records"
                    .into(),
            ));
        }
        self.states.try_reserve(1).map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record visited map exceeds compiler resources".into(),
            )
        })?;
        self.states.insert(identity, state);
        Ok(())
    }
}

fn symbol_identity(
    symbol: psi_symbols::SymbolHandle,
) -> Result<(u32, u32), MaterializationDiagnostic> {
    if !symbol.is_valid() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable nested-record path encountered an invalid nominal identity".into(),
        ));
    }
    Ok((symbol.arena_index(), symbol.generation()))
}

fn nested_path_reports_match_for_replay(
    left: &ConventionalNestedRecordSumPathLayoutReport,
    right: &ConventionalNestedRecordSumPathLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && field_occurrence_matches(
            &left.outer_field,
            left.outer_member_identity,
            &right.outer_field,
            right.outer_member_identity,
        )
        && layout_plan_reports_match_for_replay(&left.inner_layout, &right.inner_layout)
        && left.child_sum_layouts.len() == right.child_sum_layouts.len()
        && left
            .child_sum_layouts
            .iter()
            .zip(&right.child_sum_layouts)
            .all(|(left, right)| {
                field_occurrence_matches(
                    &left.field,
                    left.member_identity,
                    &right.field,
                    right.member_identity,
                ) && conventional_sum_layout_reports_match_for_replay(&left.layout, &right.layout)
            })
}

fn depth_two_path_reports_match_for_replay(
    left: &ConventionalDepthTwoRecordSumPathLayoutReport,
    right: &ConventionalDepthTwoRecordSumPathLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && field_occurrence_matches(
            &left.outer_field,
            left.outer_member_identity,
            &right.outer_field,
            right.outer_member_identity,
        )
        && nested_path_reports_match_for_replay(&left.middle_path, &right.middle_path)
}

fn depth_three_path_reports_match_for_replay(
    left: &ConventionalDepthThreeRecordSumPathLayoutReport,
    right: &ConventionalDepthThreeRecordSumPathLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && field_occurrence_matches(
            &left.outer_field,
            left.outer_member_identity,
            &right.outer_field,
            right.outer_member_identity,
        )
        && depth_two_path_reports_match_for_replay(&left.depth_two_path, &right.depth_two_path)
}

fn depth_three_paths_reports_match_for_replay(
    left: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    right: &ConventionalDepthThreeRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_two_paths_reports_match_for_replay(
                &left.depth_two_paths,
                &right.depth_two_paths,
            )
        })
}

fn depth_twelve_paths_reports_match_for_replay(
    left: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    right: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_eleven_paths_reports_match_for_replay(
                &left.depth_eleven_paths,
                &right.depth_eleven_paths,
            )
        })
}

fn depth_eleven_paths_reports_match_for_replay(
    left: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    right: &ConventionalDepthElevenRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_ten_paths_reports_match_for_replay(
                &left.depth_ten_paths,
                &right.depth_ten_paths,
            )
        })
}

fn depth_ten_paths_reports_match_for_replay(
    left: &ConventionalDepthTenRecordSumPathsLayoutReport,
    right: &ConventionalDepthTenRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_nine_paths_reports_match_for_replay(
                &left.depth_nine_paths,
                &right.depth_nine_paths,
            )
        })
}

fn depth_nine_paths_reports_match_for_replay(
    left: &ConventionalDepthNineRecordSumPathsLayoutReport,
    right: &ConventionalDepthNineRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_eight_paths_reports_match_for_replay(
                &left.depth_eight_paths,
                &right.depth_eight_paths,
            )
        })
}

fn depth_eight_paths_reports_match_for_replay(
    left: &ConventionalDepthEightRecordSumPathsLayoutReport,
    right: &ConventionalDepthEightRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_seven_paths_reports_match_for_replay(
                &left.depth_seven_paths,
                &right.depth_seven_paths,
            )
        })
}

fn depth_seven_paths_reports_match_for_replay(
    left: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    right: &ConventionalDepthSevenRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_six_paths_reports_match_for_replay(
                &left.depth_six_paths,
                &right.depth_six_paths,
            )
        })
}

fn depth_six_paths_reports_match_for_replay(
    left: &ConventionalDepthSixRecordSumPathsLayoutReport,
    right: &ConventionalDepthSixRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_five_paths_reports_match_for_replay(
                &left.depth_five_paths,
                &right.depth_five_paths,
            )
        })
}

fn depth_five_paths_reports_match_for_replay(
    left: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    right: &ConventionalDepthFiveRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_four_paths_reports_match_for_replay(
                &left.depth_four_paths,
                &right.depth_four_paths,
            )
        })
}

fn depth_four_paths_reports_match_for_replay(
    left: &ConventionalDepthFourRecordSumPathsLayoutReport,
    right: &ConventionalDepthFourRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && depth_three_paths_reports_match_for_replay(
                &left.depth_three_paths,
                &right.depth_three_paths,
            )
        })
}

fn depth_two_paths_reports_match_for_replay(
    left: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    right: &ConventionalDepthTwoRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && nested_paths_reports_match_for_replay(&left.middle_paths, &right.middle_paths)
        })
}

fn nested_paths_reports_match_for_replay(
    left: &ConventionalNestedRecordSumPathsLayoutReport,
    right: &ConventionalNestedRecordSumPathsLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && left.paths.len() == right.paths.len()
        && left.paths.iter().zip(&right.paths).all(|(left, right)| {
            field_occurrence_matches(
                &left.outer_field,
                left.outer_member_identity,
                &right.outer_field,
                right.outer_member_identity,
            ) && layout_plan_reports_match_for_replay(&left.inner_layout, &right.inner_layout)
                && left.child_sum_layouts.len() == right.child_sum_layouts.len()
                && left
                    .child_sum_layouts
                    .iter()
                    .zip(&right.child_sum_layouts)
                    .all(|(left, right)| {
                        field_occurrence_matches(
                            &left.field,
                            left.member_identity,
                            &right.field,
                            right.member_identity,
                        ) && conventional_sum_layout_reports_match_for_replay(
                            &left.layout,
                            &right.layout,
                        )
                    })
        })
}

fn depth_twelve_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-twelve-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_eleven_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_eleven_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthElevenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-eleven-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_ten_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_ten_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-ten-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_nine_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_nine_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthNineNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-nine-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_eight_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_eight_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthEightNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-eight-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_seven_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_seven_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthSevenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-seven-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_six_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_six_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthSixNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-six-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_five_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_five_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthFiveNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-five-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_four_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_four_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthFourNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-four-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_three_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_three_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthThreeNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-three-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.depth_two_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_three_nested_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthThreeRecordSumPathLayoutReport,
    inner: &ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-depth-three-record-sum-path.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    match path_layout.outer_member_identity {
        Some(identity) => {
            hash_byte(&mut hash, 1);
            hash_u64(&mut hash, identity);
        }
        None => {
            hash_byte(&mut hash, 0);
            hash_text(&mut hash, &path_layout.outer_field);
        }
    }
    hash_u64(
        &mut hash,
        normalized_layout_plan_report_fingerprint(&path_layout.depth_two_path.outer_layout),
    );
    hash_u64(
        &mut hash,
        inner.non_authoritative_materialization_report_fingerprint(),
    );
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_two_nested_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwoRecordSumPathLayoutReport,
    middle: &ValidatedConstRecordWithNestedSumRecordMaterialization,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-depth-two-record-sum-path.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    match path_layout.outer_member_identity {
        Some(identity) => {
            hash_byte(&mut hash, 1);
            hash_u64(&mut hash, identity);
        }
        None => {
            hash_byte(&mut hash, 0);
            hash_text(&mut hash, &path_layout.outer_field);
        }
    }
    hash_u64(
        &mut hash,
        normalized_layout_plan_report_fingerprint(&path_layout.middle_path.outer_layout),
    );
    hash_u64(
        &mut hash,
        middle.non_authoritative_materialization_report_fingerprint(),
    );
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn depth_two_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTwoNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-plural-depth-two-record-sum-paths.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.middle_paths.outer_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .middle
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn nested_record_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalNestedRecordSumPathLayoutReport,
    inner: &ValidatedConstRecordWithSumMaterialization,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-with-nested-sum-record.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    match path_layout.outer_member_identity {
        Some(identity) => {
            hash_byte(&mut hash, 1);
            hash_u64(&mut hash, identity);
        }
        None => {
            hash_byte(&mut hash, 0);
            hash_text(&mut hash, &path_layout.outer_field);
        }
    }
    hash_u64(
        &mut hash,
        normalized_layout_plan_report_fingerprint(&path_layout.inner_layout),
    );
    hash_u64(
        &mut hash,
        inner.non_authoritative_materialization_report_fingerprint(),
    );
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

fn nested_record_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
    inner_records: &[ValidatedConstNestedSumRecordOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-with-nested-sum-records.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, inner_records.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(inner_records) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.inner_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn reachability_validates_siblings_after_an_already_found_sum() {
        let tokens = Lexer::new(
            r#"
            data Choice [copy] { case Empty; }
            data Trap [copy] { choice: Choice; later: u8; }
            data Root [copy] { trap: Trap; }
            "#,
        )
        .tokenize()
        .expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let trap = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Trap")
            .expect("Trap definition");
        let trap_symbol = trap.symbol;
        let trap_name = trap.name.clone();
        let later_type = typed
            .data_members(trap)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "later" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .expect("later field");
        let root = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Root")
            .expect("Root definition");
        let trap_type = typed
            .data_members(root)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "trap" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .expect("trap field");
        assert!(matches!(
            SumReachability::new(&typed).type_contains_sum(trap_type),
            Ok(true)
        ));

        let mut recursive = typed.clone();
        recursive.type_reference_table.substitute_node(
            later_type,
            TypeReferenceNode::Named {
                symbol: trap_symbol,
                name: trap_name.clone(),
            },
        );
        assert!(
            SumReachability::new(&recursive)
                .type_contains_sum(trap_type)
                .is_err(),
            "a later recursive sibling must not hide behind an earlier sum"
        );

        let mut malformed = typed;
        malformed.type_reference_table.substitute_node(
            later_type,
            TypeReferenceNode::Named {
                symbol: psi_symbols::SymbolHandle::invalid(),
                name: trap_name,
            },
        );
        assert!(
            SumReachability::new(&malformed)
                .type_contains_sum(trap_type)
                .is_err(),
            "a later malformed nominal branch must not hide behind an earlier sum"
        );
    }
}
