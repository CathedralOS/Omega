//! Value-sensitive materialization of one record with direct conventional sum fields.

use psi_language_semantics::{DataSupplyMode, Multiplicity};
use psi_layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport, LayoutPlacementReport,
    LayoutPlanReport, MaterializationDiagnostic, conventional_sum_layout_reports_match_for_replay,
    layout_plan_reports_match_for_replay, materialize_aggregate_layout_into,
    normalized_conventional_sum_layout_report_fingerprint,
    normalized_layout_plan_report_fingerprint,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceNode};

use super::const_materializable::{
    hash_byte, hash_bytes, hash_text, hash_u64, hash_value, unique_data_by_name, validate_value,
    value_kind,
};
use super::{
    BuildTimeValue, RepeatedFieldInfo, ValidatedConstSumMaterialization, encode_typed_owned_value,
    exact_struct_fields, normalized_schema_report_fingerprint, reflected_field_layout,
    validate_const_materializable_conventional_sum,
};

/// Exact materialization custody for one direct runtime-relevant conventional
/// pure-sum field of a closed record.
#[derive(Debug)]
pub struct ValidatedConstRecordSumFieldMaterialization {
    field: String,
    field_identity: Option<u64>,
    nested_sum: ValidatedConstSumMaterialization,
}

/// Value-sensitive custody for one literal element of the sole direct
/// fixed-array-of-conventional-sums field.
#[derive(Debug)]
pub struct ValidatedConstRecordSumArrayElementMaterialization {
    literal_index: u64,
    nested_sum: ValidatedConstSumMaterialization,
}

impl ValidatedConstRecordSumArrayElementMaterialization {
    pub const fn literal_index(&self) -> u64 {
        self.literal_index
    }

    pub const fn nested_sum(&self) -> &ValidatedConstSumMaterialization {
        &self.nested_sum
    }
}

/// Exact materialization custody for one closed record containing exactly one
/// direct, nonzero literal fixed array of conventional pure sums.
#[derive(Debug)]
pub struct ValidatedConstRecordWithSumArrayMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    layout: LayoutPlanReport,
    non_authoritative_layout_report_fingerprint: u64,
    array_layout: ConventionalSumArrayFieldLayoutReport,
    elements: Vec<ValidatedConstRecordSumArrayElementMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithSumArrayMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub const fn array_layout(&self) -> &ConventionalSumArrayFieldLayoutReport {
        &self.array_layout
    }

    pub fn elements(&self) -> &[ValidatedConstRecordSumArrayElementMaterialization] {
        &self.elements
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Reconstruct the exact outer layout, compact array layout, every indexed
    /// selected sum, and the staged whole-record image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        array_layout: &ConventionalSumArrayFieldLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array record invocation drifted from retained custody"
                    .into(),
            ));
        }
        let layout_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        if layout_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !layout_plan_reports_match_for_replay(layout, &self.layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array outer layout drifted from retained custody".into(),
            ));
        }
        if !sum_array_layouts_match_for_replay(array_layout, &self.array_layout) {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable compact sum-array layout drifted from retained custody".into(),
            ));
        }
        let replayed = derive_record_with_sum_array_bytes(
            typed,
            schema_name,
            layout,
            array_layout,
            value,
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || !sum_array_elements_match(&replayed.elements, &self.elements)
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable indexed sum-array custody drifted from exact replay".into(),
            ));
        }
        let fingerprint = non_authoritative_record_with_sum_array_materialization_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            layout_fingerprint,
            array_layout,
            &replayed.elements,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array report fingerprint drifted from exact replay".into(),
            ));
        }
        Ok(())
    }

    /// Replay before atomically copying the complete outer record image.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.layout,
            &self.array_layout,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable sum-array record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

impl ValidatedConstRecordSumFieldMaterialization {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn field_identity(&self) -> Option<u64> {
        self.field_identity
    }

    /// Complete retained conventional layout, selected case, value, byte, and
    /// report custody for this direct sum field.
    pub const fn nested_sum(&self) -> &ValidatedConstSumMaterialization {
        &self.nested_sum
    }
}

/// Exact materialization custody for one closed record containing one or more
/// direct, runtime-relevant conventional pure-sum fields.
///
/// This deliberately distinct carrier keeps arrays of sums, recursively nested
/// sums, mixed data shapes, and target-dependent sum geometry outside the first
/// nested-sum rung. It does not implement `Clone`: replay reconstructs every
/// outer and nested fact from the caller's current typed program.
#[derive(Debug)]
pub struct ValidatedConstRecordWithSumMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    layout: LayoutPlanReport,
    non_authoritative_layout_report_fingerprint: u64,
    nested_sums: Vec<ValidatedConstRecordSumFieldMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithSumMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn non_authoritative_schema_report_fingerprint(&self) -> u64 {
        self.non_authoritative_schema_report_fingerprint
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }

    /// Complete authored-order custody for every direct sum field.
    pub fn nested_sums(&self) -> &[ValidatedConstRecordSumFieldMaterialization] {
        &self.nested_sums
    }

    /// Compatibility accessor for the singular API. Generalized consumers
    /// should use [`Self::nested_sums`].
    pub fn nested_sum_field(&self) -> &str {
        self.nested_sums[0].field()
    }

    /// Compatibility accessor for the singular API.
    pub fn nested_sum_field_identity(&self) -> Option<u64> {
        self.nested_sums[0].field_identity()
    }

    /// Compatibility accessor for the singular API.
    pub fn nested_sum(&self) -> &ValidatedConstSumMaterialization {
        self.nested_sums[0].nested_sum()
    }

    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Independently replay the outer layout and the complete caller-supplied
    /// conventional nested-sum layout. Compact report fingerprints are checked
    /// only after exact hash-free layout comparisons.
    pub fn replay_against_sum_fields(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        nested_sum_layouts: &[ConventionalSumFieldLayoutReport],
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum record schema `{schema_name}` does not match retained schema `{}`",
                self.schema_name
            )));
        }
        if value != &self.value {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record value drifted from retained custody".into(),
            ));
        }
        if byte_order != self.byte_order {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record target byte order drifted from retained custody"
                    .into(),
            ));
        }
        let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        if layout_report_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !layout_plan_reports_match_for_replay(layout, &self.layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum outer layout drifted from retained custody".into(),
            ));
        }
        validate_supplied_nested_rows_against_retained(
            typed,
            nested_sum_layouts,
            &self.nested_sums,
            byte_order,
        )?;

        let replayed = derive_record_with_sum_bytes(
            typed,
            schema_name,
            layout,
            nested_sum_layouts,
            value,
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || !nested_sum_fields_match(&replayed.nested_sums, &self.nested_sums)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum field identity drifted from retained custody"
                    .into(),
            ));
        }
        if replayed.bytes != self.bytes {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record bytes drifted from exact zero-initialized replay"
                    .into(),
            ));
        }
        let materialization_report_fingerprint =
            non_authoritative_record_with_sum_materialization_report_fingerprint(
                schema_name,
                replayed.schema_report_fingerprint,
                layout_report_fingerprint,
                &replayed.nested_sums,
                byte_order,
                value,
                &replayed.bytes,
            );
        if materialization_report_fingerprint
            != self.non_authoritative_materialization_report_fingerprint
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record report fingerprint drifted from exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay the original singular-row surface without weakening plural
    /// custody. Records with more than one direct sum reject this wrapper.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        nested_sum_layout: &ConventionalSumLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        let [nested] = self.nested_sums.as_slice() else {
            return Err(MaterializationDiagnostic(
                "singular nested-sum replay cannot discard multiple retained field rows".into(),
            ));
        };
        self.replay_against_sum_fields(
            typed,
            schema_name,
            layout,
            &[ConventionalSumFieldLayoutReport {
                field: nested.field.clone(),
                member_identity: nested.field_identity,
                layout: nested_sum_layout.clone(),
            }],
            value,
            byte_order,
        )
    }

    /// Replay both retained layouts before atomically copying the exact outer
    /// bytes. Rejection and a short destination leave `destination` unchanged.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against_sum_fields(
            typed,
            &self.schema_name,
            &self.layout,
            &self
                .nested_sums
                .iter()
                .map(|row| ConventionalSumFieldLayoutReport {
                    field: row.field.clone(),
                    member_identity: row.field_identity,
                    layout: row.nested_sum.layout().clone(),
                })
                .collect::<Vec<_>>(),
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Validate every direct conventional pure-sum field inside one closed
/// non-generic `[copy]` record. The outer record uses its exact validated
/// `LayoutPlanReport`; each nested sum uses the compiler-owned conventional
/// runtime layout and cannot acquire programmable tag/case placement. The
/// supplied rows must be the complete authored-order direct-sum field set.
pub fn validate_const_materializable_record_with_conventional_sums(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    nested_sum_layouts: &[ConventionalSumFieldLayoutReport],
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithSumMaterialization, MaterializationDiagnostic> {
    let derived = derive_record_with_sum_bytes(
        typed,
        schema_name,
        layout,
        nested_sum_layouts,
        value,
        byte_order,
    )?;
    let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
    let materialization_report_fingerprint =
        non_authoritative_record_with_sum_materialization_report_fingerprint(
            schema_name,
            derived.schema_report_fingerprint,
            layout_report_fingerprint,
            &derived.nested_sums,
            byte_order,
            value,
            &derived.bytes,
        );
    Ok(ValidatedConstRecordWithSumMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        layout: layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint,
        nested_sums: derived.nested_sums,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_report_fingerprint,
    })
}

/// Preserve the original singular projection surface. It remains fail-closed
/// when the record now requires more than one direct sum-field row.
pub fn validate_const_materializable_record_with_conventional_sum(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    nested_sum_layout: &ConventionalSumLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithSumMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let direct_sums = typed
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .filter_map(
            |field| match exact_named_data(typed, field.type_reference) {
                Ok(Some(named))
                    if DataDefinition::shape_kind_from_members(typed.data_members(named))
                        == DataShapeKind::Enum =>
                {
                    Some(Ok(field))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    if direct_sums.is_empty() {
        return validate_const_materializable_record_with_conventional_sums(
            typed,
            schema_name,
            layout,
            &[],
            value,
            byte_order,
        );
    }
    let [field] = direct_sums.as_slice() else {
        return Err(MaterializationDiagnostic(format!(
            "singular nested-sum validation requires exactly one direct runtime-relevant pure-sum field; `{schema_name}` has {}",
            direct_sums.len()
        )));
    };
    validate_const_materializable_record_with_conventional_sums(
        typed,
        schema_name,
        layout,
        &[ConventionalSumFieldLayoutReport {
            field: field.name.to_string(),
            member_identity: field.identity,
            layout: nested_sum_layout.clone(),
        }],
        value,
        byte_order,
    )
}

struct DerivedRecordWithSumMaterialization {
    schema_report_fingerprint: u64,
    nested_sums: Vec<ValidatedConstRecordSumFieldMaterialization>,
    bytes: Vec<u8>,
}

struct DerivedRecordWithSumArrayMaterialization {
    schema_report_fingerprint: u64,
    elements: Vec<ValidatedConstRecordSumArrayElementMaterialization>,
    bytes: Vec<u8>,
}

pub(super) struct EncodedOuterField {
    pub(super) name: String,
    pub(super) identity: Option<u64>,
    pub(super) size: u64,
    pub(super) align: u64,
    pub(super) repeated: Option<RepeatedFieldInfo>,
    pub(super) bytes: Vec<u8>,
}

fn derive_record_with_sum_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    nested_sum_layouts: &[ConventionalSumFieldLayoutReport],
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedRecordWithSumMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum layout schema report fingerprint does not match `{schema_name}`"
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

    let mut direct_sums = Vec::new();
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
        let Some(named) = exact_named_data(typed, field.type_reference)? else {
            continue;
        };
        match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
            DataShapeKind::Enum => {
                if field.relevance.is_erased() {
                    continue;
                }
                direct_sums.push((field, named));
            }
            DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "field `{}` uses a mixed common-field/case shape, which is outside the conventional pure-sum rung",
                    field.name
                )));
            }
            DataShapeKind::Empty | DataShapeKind::Record => {}
        }
    }
    if direct_sums.is_empty() {
        return Err(
        MaterializationDiagnostic(
            "nested-sum ConstMaterializable requires at least one direct runtime-relevant pure-sum field"
                .into(),
        ));
    }
    if nested_sum_layouts.len() != direct_sums.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum rows contain {} field(s), expected the complete authored-order set of {}",
            nested_sum_layouts.len(),
            direct_sums.len()
        )));
    }
    let mut nested_sums = Vec::with_capacity(direct_sums.len());
    for ((field, sum_data), row) in direct_sums.iter().zip(nested_sum_layouts) {
        if !field_occurrence_matches(
            &row.field,
            row.member_identity,
            field.name.as_str(),
            field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum row for `{}` is missing, duplicated, or out of authored field order",
                field.name
            )));
        }
        let sum_value = supplied
            .get(field.name.as_str())
            .expect("complete record value checked above");
        let nested_sum = validate_const_materializable_conventional_sum(
            typed,
            sum_data.name.as_str(),
            &row.layout,
            sum_value,
            byte_order,
        )?;
        nested_sums.push(ValidatedConstRecordSumFieldMaterialization {
            field: field.name.to_string(),
            field_identity: field.identity,
            nested_sum,
        });
    }

    let mut encoded_fields = Vec::new();
    let mut active = vec![data.symbol];
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete record value checked above");
        if let Some(nested_row) = nested_sums.iter().find(|row| {
            field_occurrence_matches(
                &row.field,
                row.field_identity,
                field.name.as_str(),
                field.identity,
            )
        }) {
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: nested_row.nested_sum.layout().size,
                align: nested_row.nested_sum.layout().align,
                repeated: None,
                bytes: nested_row.nested_sum.bytes().to_vec(),
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
    validate_outer_layout(layout, &encoded_fields)?;
    let byte_len = usize::try_from(layout.size.expect("validated fixed extent")).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable nested-sum record extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = vec![0; byte_len];
    let mut schemas = Vec::with_capacity(encoded_fields.len());
    let mut values = Vec::with_capacity(encoded_fields.len());
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
    materialize_aggregate_layout_into(layout, &schemas, &values, &mut bytes)?;

    Ok(DerivedRecordWithSumMaterialization {
        schema_report_fingerprint,
        nested_sums,
        bytes,
    })
}

/// Validate the compact layout of the sole direct fixed-array-of-sums field,
/// retain each selected element independently, and stage one complete outer
/// record image.
pub fn validate_const_materializable_record_with_conventional_sum_array(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithSumArrayMaterialization, MaterializationDiagnostic> {
    let derived = derive_record_with_sum_array_bytes(
        typed,
        schema_name,
        layout,
        array_layout,
        value,
        byte_order,
    )?;
    let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
    let materialization_report_fingerprint =
        non_authoritative_record_with_sum_array_materialization_fingerprint(
            schema_name,
            derived.schema_report_fingerprint,
            layout_report_fingerprint,
            array_layout,
            &derived.elements,
            byte_order,
            value,
            &derived.bytes,
        );
    Ok(ValidatedConstRecordWithSumArrayMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        layout: layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint,
        array_layout: array_layout.clone(),
        elements: derived.elements,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_report_fingerprint,
    })
}

fn derive_record_with_sum_array_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedRecordWithSumArrayMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable sum-array layout schema report fingerprint does not match `{schema_name}`"
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

    let mut selected_array = None;
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
        match typed
            .type_reference_table
            .type_reference(field.type_reference)
        {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                if let Some(named) = exact_named_data(typed, *element_type)? {
                    match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
                        DataShapeKind::Enum => {
                            if *length == 0 {
                                return Err(MaterializationDiagnostic(format!(
                                    "sum-array field `{}` must have nonzero literal length",
                                    field.name
                                )));
                            }
                            if selected_array.is_some() {
                                return Err(MaterializationDiagnostic(
                                    "ConstMaterializable sum-array rung permits exactly one direct array field"
                                        .into(),
                                ));
                            }
                            selected_array = Some((field, named, *length));
                        }
                        DataShapeKind::Mixed => {
                            return Err(MaterializationDiagnostic(format!(
                                "field `{}` uses an array of mixed common-field/case elements",
                                field.name
                            )));
                        }
                        DataShapeKind::Empty | DataShapeKind::Record => {}
                    }
                }
            }
            _ => {
                if let Some(named) = exact_named_data(typed, field.type_reference)?
                    && DataDefinition::shape_kind_from_members(typed.data_members(named))
                        == DataShapeKind::Enum
                {
                    return Err(MaterializationDiagnostic(
                        "ConstMaterializable sum-array rung does not combine direct sum fields with its array field"
                            .into(),
                    ));
                }
            }
        }
    }
    let (array_field, sum_data, element_count) = selected_array.ok_or_else(|| {
        MaterializationDiagnostic(
            "ConstMaterializable sum-array rung requires exactly one direct nonzero literal fixed-array-of-sums field"
                .into(),
        )
    })?;
    if !field_occurrence_matches(
        &array_layout.field,
        array_layout.member_identity,
        array_field.name.as_str(),
        array_field.identity,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable compact sum-array row does not name exact field `{}`",
            array_field.name
        )));
    }
    let element_count_u64 = u64::try_from(element_count).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable sum-array count exceeds canonical report width".into(),
        )
    })?;
    if array_layout.element_count != element_count_u64
        || array_layout.element_stride != array_layout.element_layout.size
    {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable compact sum-array count/stride drifted for `{}`",
            array_field.name
        )));
    }
    let array_value = supplied
        .get(array_field.name.as_str())
        .expect("complete record value checked above");
    let BuildTimeValue::Array(element_values) = array_value else {
        return Err(MaterializationDiagnostic(format!(
            "value.{} is not a fixed array",
            array_field.name
        )));
    };
    if element_values.len() != element_count {
        return Err(MaterializationDiagnostic(format!(
            "value.{} has {} elements, expected {element_count}",
            array_field.name,
            element_values.len()
        )));
    }
    let mut elements = Vec::new();
    elements.try_reserve_exact(element_count).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable sum-array element custody exceeds compiler resources".into(),
        )
    })?;
    let total_size = array_layout
        .element_stride
        .checked_mul(array_layout.element_count)
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable sum-array physical extent overflows".into(),
            )
        })?;
    let total_size_usize = usize::try_from(total_size).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable sum-array physical extent exceeds compiler host".into(),
        )
    })?;
    let mut array_bytes = Vec::new();
    array_bytes
        .try_reserve_exact(total_size_usize)
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable sum-array staged bytes exceed compiler resources".into(),
            )
        })?;
    for (index, element_value) in element_values.iter().enumerate() {
        let nested_sum = validate_const_materializable_conventional_sum(
            typed,
            sum_data.name.as_str(),
            &array_layout.element_layout,
            element_value,
            byte_order,
        )?;
        array_bytes.extend_from_slice(nested_sum.bytes());
        elements.push(ValidatedConstRecordSumArrayElementMaterialization {
            literal_index: u64::try_from(index).map_err(|_| {
                MaterializationDiagnostic(
                    "ConstMaterializable sum-array index exceeds canonical width".into(),
                )
            })?,
            nested_sum,
        });
    }
    if array_bytes.len() != total_size_usize {
        return Err(MaterializationDiagnostic(format!(
            "value.{} encoded to {} bytes, expected {total_size}",
            array_field.name,
            array_bytes.len()
        )));
    }

    let mut encoded_fields = Vec::new();
    let mut active = vec![data.symbol];
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete record value checked above");
        if field_occurrence_matches(
            field.name.as_str(),
            field.identity,
            array_field.name.as_str(),
            array_field.identity,
        ) {
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: total_size,
                align: array_layout.element_layout.align,
                repeated: Some(RepeatedFieldInfo {
                    element_size: array_layout.element_stride,
                    element_align: array_layout.element_layout.align,
                    element_count: array_layout.element_count,
                }),
                bytes: std::mem::take(&mut array_bytes),
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
    validate_outer_layout(layout, &encoded_fields)?;
    let byte_len = usize::try_from(layout.size.expect("validated fixed extent")).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable sum-array record extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = vec![0; byte_len];
    let mut schemas = Vec::with_capacity(encoded_fields.len());
    let mut values = Vec::with_capacity(encoded_fields.len());
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
    materialize_aggregate_layout_into(layout, &schemas, &values, &mut bytes)?;

    Ok(DerivedRecordWithSumArrayMaterialization {
        schema_report_fingerprint,
        elements,
        bytes,
    })
}

pub(super) fn validate_outer_record_owner(
    typed: &TypedTrees,
    data: &DataDefinition,
) -> Result<(), MaterializationDiagnostic> {
    if !data.symbol.is_valid()
        || data.supply_mode != DataSupplyMode::CheckedShape
        || !data.type_parameters.is_empty()
        || !data.lifetime_parameters.is_empty()
        || data.generic_instance.is_some()
        || data.quotient.is_some()
    {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum record `{}` is generic, opaque, quotient, or lacks one exact closed checked-shape identity",
            data.name
        )));
    }
    if data.properties.multiplicity != Multiplicity::Unrestricted {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum record `{}` is not `[copy]`",
            data.name
        )));
    }
    if DataDefinition::shape_kind_from_members(typed.data_members(data)) != DataShapeKind::Record {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum owner `{}` is not one record",
            data.name
        )));
    }
    Ok(())
}

pub(super) fn exact_named_data<'a>(
    typed: &'a TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Result<Option<&'a DataDefinition>, MaterializationDiagnostic> {
    if typed.primitive_type_reference(type_reference).is_some() {
        return Ok(None);
    }
    let TypeReferenceNode::Named { symbol, name } =
        typed.type_reference_table.type_reference(type_reference)
    else {
        return Ok(None);
    };
    if !symbol.is_valid() {
        return Err(MaterializationDiagnostic(format!(
            "named field type `{name}` has no exact nominal identity"
        )));
    }
    let mut matches = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == *symbol);
    let Some(data) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() || data.name.as_str() != name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "named field type `{name}` has ambiguous or mismatched nominal identity"
        )));
    }
    Ok(Some(data))
}

pub(super) fn validate_outer_layout(
    layout: &LayoutPlanReport,
    fields: &[EncodedOuterField],
) -> Result<(), MaterializationDiagnostic> {
    let size = layout.size.ok_or_else(|| {
        MaterializationDiagnostic(
            "ConstMaterializable nested-sum record requires one exact fixed layout extent".into(),
        )
    })?;
    if layout.align == 0 || !layout.align.is_power_of_two() || !size.is_multiple_of(layout.align) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum record extent {size} is inconsistent with alignment {}",
            layout.align
        )));
    }
    let required_align = fields.iter().map(|field| field.align).max().unwrap_or(1);
    if layout.align < required_align {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum outer alignment {} is below field alignment {required_align}",
            layout.align
        )));
    }

    let mut expected_offsets = Vec::with_capacity(fields.len());
    for field in fields {
        let entries = layout
            .entries
            .iter()
            .filter(|entry| match field.identity {
                Some(identity) => entry.member_identity == Some(identity),
                None => entry.member_identity.is_none() && entry.field == field.name,
            })
            .collect::<Vec<_>>();
        let [entry] = entries.as_slice() else {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum field `{}` requires exactly one whole placement",
                field.name
            )));
        };
        let LayoutPlacementReport::At { offset } = entry.placement else {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum field `{}` requires one whole `At` placement",
                field.name
            )));
        };
        if !offset.is_multiple_of(field.align) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum field `{}` offset {offset} violates alignment {}",
                field.name, field.align
            )));
        }
        expected_offsets.push(offset);
    }
    if layout.offsets.as_deref() != Some(expected_offsets.as_slice()) {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable nested-sum outer offsets do not replay exact declaration-order placements"
                .into(),
        ));
    }
    Ok(())
}

fn validate_supplied_nested_rows_against_retained(
    typed: &TypedTrees,
    supplied: &[ConventionalSumFieldLayoutReport],
    retained: &[ValidatedConstRecordSumFieldMaterialization],
    byte_order: ByteOrder,
) -> Result<(), MaterializationDiagnostic> {
    if supplied.len() != retained.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable nested-sum rows contain {} field(s), retained custody requires {}",
            supplied.len(),
            retained.len()
        )));
    }
    for (row, retained) in supplied.iter().zip(retained) {
        if !field_occurrence_matches(
            &row.field,
            row.member_identity,
            retained.field(),
            retained.field_identity(),
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum row `{}` is duplicated or out of retained authored field order",
                row.field
            )));
        }
        let nested = retained.nested_sum();
        let layout_fingerprint = normalized_conventional_sum_layout_report_fingerprint(&row.layout);
        if layout_fingerprint != nested.non_authoritative_layout_report_fingerprint()
            || !conventional_sum_layout_reports_match_for_replay(&row.layout, nested.layout())
        {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested conventional sum layout for field `{}` drifted from retained custody",
                row.field
            )));
        }
        nested.replay_against(
            typed,
            nested.schema_name(),
            &row.layout,
            nested.value(),
            byte_order,
        )?;
    }
    Ok(())
}

fn nested_sum_fields_match(
    left: &[ValidatedConstRecordSumFieldMaterialization],
    right: &[ValidatedConstRecordSumFieldMaterialization],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            field_occurrence_matches(
                &left.field,
                left.field_identity,
                &right.field,
                right.field_identity,
            ) && left.nested_sum.schema_name() == right.nested_sum.schema_name()
                && left.nested_sum.value() == right.nested_sum.value()
                && left.nested_sum.selected_case_identity()
                    == right.nested_sum.selected_case_identity()
                && left.nested_sum.selected_case_ordinal()
                    == right.nested_sum.selected_case_ordinal()
                && left.nested_sum.bytes() == right.nested_sum.bytes()
                && conventional_sum_layout_reports_match_for_replay(
                    left.nested_sum.layout(),
                    right.nested_sum.layout(),
                )
                && left
                    .nested_sum
                    .non_authoritative_materialization_report_fingerprint()
                    == right
                        .nested_sum
                        .non_authoritative_materialization_report_fingerprint()
        })
}

pub(super) fn field_occurrence_matches(
    left_name: &str,
    left_identity: Option<u64>,
    right_name: &str,
    right_identity: Option<u64>,
) -> bool {
    match (left_identity, right_identity) {
        (Some(left), Some(right)) => left == right,
        (None, None) => left_name == right_name,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn sum_array_layouts_match_for_replay(
    left: &ConventionalSumArrayFieldLayoutReport,
    right: &ConventionalSumArrayFieldLayoutReport,
) -> bool {
    field_occurrence_matches(
        &left.field,
        left.member_identity,
        &right.field,
        right.member_identity,
    ) && left.element_count == right.element_count
        && left.element_stride == right.element_stride
        && normalized_conventional_sum_layout_report_fingerprint(&left.element_layout)
            == normalized_conventional_sum_layout_report_fingerprint(&right.element_layout)
        && conventional_sum_layout_reports_match_for_replay(
            &left.element_layout,
            &right.element_layout,
        )
}

fn sum_array_elements_match(
    left: &[ValidatedConstRecordSumArrayElementMaterialization],
    right: &[ValidatedConstRecordSumArrayElementMaterialization],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.literal_index == right.literal_index
                && left.nested_sum.schema_name() == right.nested_sum.schema_name()
                && left.nested_sum.value() == right.nested_sum.value()
                && left.nested_sum.selected_case_identity()
                    == right.nested_sum.selected_case_identity()
                && left.nested_sum.selected_case_ordinal()
                    == right.nested_sum.selected_case_ordinal()
                && left.nested_sum.bytes() == right.nested_sum.bytes()
                && conventional_sum_layout_reports_match_for_replay(
                    left.nested_sum.layout(),
                    right.nested_sum.layout(),
                )
                && left
                    .nested_sum
                    .non_authoritative_materialization_report_fingerprint()
                    == right
                        .nested_sum
                        .non_authoritative_materialization_report_fingerprint()
        })
}

fn non_authoritative_record_with_sum_array_materialization_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    layout_report_fingerprint: u64,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    elements: &[ValidatedConstRecordSumArrayElementMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-with-sum-array.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, layout_report_fingerprint);
    match array_layout.member_identity {
        Some(identity) => {
            hash_byte(&mut hash, 1);
            hash_u64(&mut hash, identity);
        }
        None => {
            hash_byte(&mut hash, 0);
            hash_text(&mut hash, &array_layout.field);
        }
    }
    hash_u64(&mut hash, array_layout.element_count);
    hash_u64(&mut hash, array_layout.element_stride);
    hash_u64(
        &mut hash,
        normalized_conventional_sum_layout_report_fingerprint(&array_layout.element_layout),
    );
    hash_u64(&mut hash, elements.len() as u64);
    for element in elements {
        hash_u64(&mut hash, element.literal_index);
        hash_u64(
            &mut hash,
            element
                .nested_sum
                .non_authoritative_layout_report_fingerprint(),
        );
        match element.nested_sum.selected_case_identity() {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => hash_byte(&mut hash, 0),
        }
        hash_u64(
            &mut hash,
            u64::from(element.nested_sum.selected_case_ordinal()),
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

fn non_authoritative_record_with_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    layout_report_fingerprint: u64,
    nested_sums: &[ValidatedConstRecordSumFieldMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.const-materializable-record-with-sum.v2");
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, layout_report_fingerprint);
    hash_u64(&mut hash, nested_sums.len() as u64);
    for nested in nested_sums {
        match nested.field_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &nested.field);
            }
        }
        hash_u64(
            &mut hash,
            nested
                .nested_sum
                .non_authoritative_layout_report_fingerprint(),
        );
        match nested.nested_sum.selected_case_identity() {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => hash_byte(&mut hash, 0),
        }
        hash_u64(
            &mut hash,
            u64::from(nested.nested_sum.selected_case_ordinal()),
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
    use psi_layout_plans::{
        ConventionalSumCaseLayoutReport, ConventionalSumPayloadFieldLayoutReport,
        LayoutFieldEntryReport,
    };
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    use crate::layout_plans::{checked_align_up, reflected_nested_member_layout};

    const SOURCE: &str = r#"
        data Choice [copy] {
            case Empty;
            case Small(value: u16);
            case Wide(code: u32, flag: u8);
        }
        data Envelope [copy] { prefix: u8; choice: Choice; suffix: u16; }
        data TwoChoices [copy] { first: Choice; second: Choice; }
        data ChoiceArray [copy] { choices: [Choice; 2]; }
        data ZeroChoiceArray [copy] { choices: [Choice; 0]; }
        data TwoChoiceArrays [copy] { first: [Choice; 1]; second: [Choice; 1]; }
        data NestedChoiceArray [copy] { choices: [[Choice; 2]; 1]; }
        data InnerEnvelope [copy] { choice: Choice; }
        data DeepEnvelope [copy] { inner: InnerEnvelope; }
        data MixedChoice [copy] { common: u8; case Empty; case Number(value: u8); }
        data MixedEnvelope [copy] { choice: MixedChoice; }
        data FloatingChoice [copy] { case Empty; case Number(value: f64); }
        data FloatingEnvelope [copy] { choice: FloatingChoice; }
        data BorrowedEnvelope [copy] { choice: Choice; borrowed: &u8; }
        data TextEnvelope [copy] { choice: Choice; text: Text; }
        trait Shape { machine code(&self) -> u8; }
        data DynamicEnvelope [copy] { choice: Choice; shape: dyn Shape; }
        data Carrier [copy] { case Unit; }
        proposition same(left: Carrier, right: Carrier) = left == right;
        data Quotient = Carrier % same;
        data QuotientEnvelope [copy] { choice: Choice; quotient: Quotient; }
        data GenericEnvelope<T [copy]> [copy] { choice: Choice; value: T; }
    "#;

    fn typed() -> TypedTrees {
        let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn conventional_sum_layout(
        typed: &TypedTrees,
        schema_name: &str,
    ) -> ConventionalSumLayoutReport {
        let data = unique_data_by_name(typed, schema_name).expect("sum definition");
        let cases = typed
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant),
                DataMember::Field(_) => None,
            })
            .collect::<Vec<_>>();
        let mut maximum_align = 1;
        let mut shapes = Vec::new();
        for case in &cases {
            let fields = typed
                .data_payload_fields(case)
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    let (size, align) = reflected_nested_member_layout(
                        typed,
                        field.type_reference,
                        &mut vec![data.symbol],
                    )
                    .expect("fixed payload field");
                    maximum_align = maximum_align.max(align);
                    (field, size, align)
                })
                .collect::<Vec<_>>();
            shapes.push(fields);
        }
        let payload_base = checked_align_up(4, maximum_align).expect("payload base");
        let mut maximum_end = 4;
        let cases = cases
            .iter()
            .zip(shapes)
            .enumerate()
            .map(|(ordinal, (case, fields))| {
                let mut offset = payload_base;
                let payload_fields = fields
                    .into_iter()
                    .map(|(field, size, align)| {
                        offset = checked_align_up(offset, align).expect("payload alignment");
                        let report = ConventionalSumPayloadFieldLayoutReport {
                            field: field.name.to_string(),
                            member_identity: field.identity,
                            offset,
                            size,
                            align,
                        };
                        offset += size;
                        report
                    })
                    .collect();
                maximum_end = maximum_end.max(offset);
                ConventionalSumCaseLayoutReport {
                    case: case.name.to_string(),
                    member_identity: case.identity,
                    ordinal: ordinal as u32,
                    payload_fields,
                }
            })
            .collect();
        let align = 4.max(maximum_align);
        ConventionalSumLayoutReport {
            schema_report_fingerprint: normalized_schema_report_fingerprint(typed, data),
            tag_offset: 0,
            tag_size: 4,
            tag_align: 4,
            cases,
            size: checked_align_up(maximum_end, align).expect("sum extent"),
            align,
        }
    }

    fn outer_layout(
        typed: &TypedTrees,
        schema_name: &str,
        offsets: &[u64],
        size: u64,
        align: u64,
    ) -> LayoutPlanReport {
        let data = unique_data_by_name(typed, schema_name).expect("record definition");
        let fields = typed
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), offsets.len());
        LayoutPlanReport {
            schema_report_fingerprint: normalized_schema_report_fingerprint(typed, data),
            entries: fields
                .iter()
                .zip(offsets)
                .map(|(field, offset)| LayoutFieldEntryReport {
                    field: field.name.to_string(),
                    member_identity: field.identity,
                    placement: LayoutPlacementReport::At { offset: *offset },
                })
                .collect(),
            offsets: Some(offsets.to_vec()),
            size: Some(size),
            align,
        }
    }

    fn choice_value() -> BuildTimeValue {
        BuildTimeValue::Case {
            variant: "Wide".into(),
            payload: vec![
                ("code".into(), BuildTimeValue::Int(0x1122_3344)),
                ("flag".into(), BuildTimeValue::Int(9)),
            ],
        }
    }

    fn small_choice_value(value: i64) -> BuildTimeValue {
        BuildTimeValue::Case {
            variant: "Small".into(),
            payload: vec![("value".into(), BuildTimeValue::Int(value))],
        }
    }

    fn direct_sum_rows(
        outer: &LayoutPlanReport,
        rows: Vec<(&str, ConventionalSumLayoutReport)>,
    ) -> Vec<ConventionalSumFieldLayoutReport> {
        rows.into_iter()
            .map(|(field, layout)| {
                let entry = outer
                    .entries
                    .iter()
                    .find(|entry| entry.field == field)
                    .expect("direct sum field has one outer row");
                ConventionalSumFieldLayoutReport {
                    field: field.into(),
                    member_identity: entry.member_identity,
                    layout,
                }
            })
            .collect()
    }

    fn sum_array_row(
        outer: &LayoutPlanReport,
        field: &str,
        element_count: u64,
        element_layout: ConventionalSumLayoutReport,
    ) -> ConventionalSumArrayFieldLayoutReport {
        let entry = outer
            .entries
            .iter()
            .find(|entry| entry.field == field)
            .expect("sum array field has one outer row");
        ConventionalSumArrayFieldLayoutReport {
            field: field.into(),
            member_identity: entry.member_identity,
            element_count,
            element_stride: element_layout.size,
            element_layout,
        }
    }

    fn envelope_value() -> BuildTimeValue {
        BuildTimeValue::Struct {
            type_name: "Envelope".into(),
            fields: vec![
                ("prefix".into(), BuildTimeValue::Int(7)),
                ("choice".into(), choice_value()),
                ("suffix".into(), BuildTimeValue::Int(0x5566)),
            ],
        }
    }

    #[test]
    fn one_nested_sum_retains_both_layouts_selection_byte_order_and_zero_padding() {
        let typed = typed();
        let nested = conventional_sum_layout(&typed, "Choice");
        let outer = outer_layout(&typed, "Envelope", &[0, 4, 18], 20, 4);
        let value = envelope_value();

        let little = validate_const_materializable_record_with_conventional_sum(
            &typed,
            "Envelope",
            &outer,
            &nested,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("one direct pure-sum field should materialize");
        assert_eq!(little.nested_sum_field(), "choice");
        assert_eq!(little.nested_sum().schema_name(), "Choice");
        assert_eq!(little.nested_sum().selected_case_ordinal(), 2);
        assert_ne!(little.non_authoritative_schema_report_fingerprint(), 0);
        assert_ne!(little.non_authoritative_layout_report_fingerprint(), 0);
        assert_ne!(
            little.non_authoritative_materialization_report_fingerprint(),
            0
        );
        assert_eq!(
            little.bytes(),
            &[
                7, 0, 0, 0, 2, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 9, 0, 0, 0, 0, 0, 0x66, 0x55,
            ]
        );
        little
            .replay_against(
                &typed,
                "Envelope",
                &outer,
                &nested,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect("both exact layouts replay");

        let big = validate_const_materializable_record_with_conventional_sum(
            &typed,
            "Envelope",
            &outer,
            &nested,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("target byte order remains explicit");
        assert_eq!(
            big.bytes(),
            &[
                7, 0, 0, 0, 0, 0, 0, 2, 0x11, 0x22, 0x33, 0x44, 9, 0, 0, 0, 0, 0, 0x55, 0x66,
            ]
        );
        assert_ne!(
            little.non_authoritative_materialization_report_fingerprint(),
            big.non_authoritative_materialization_report_fingerprint()
        );

        let mut destination = [0xa5; 24];
        little
            .apply(&typed, &mut destination)
            .expect("exact evidence copies atomically");
        assert_eq!(&destination[..20], little.bytes());
        assert_eq!(&destination[20..], &[0xa5; 4]);
    }

    #[test]
    fn replay_rejects_outer_nested_selection_byte_and_compact_coordinate_drift_atomically() {
        let typed = typed();
        let nested = conventional_sum_layout(&typed, "Choice");
        let outer = outer_layout(&typed, "Envelope", &[0, 4, 18], 20, 4);
        let value = envelope_value();
        let carrier = validate_const_materializable_record_with_conventional_sum(
            &typed,
            "Envelope",
            &outer,
            &nested,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("fixture should validate");

        let mut wrong_outer = outer.clone();
        wrong_outer.entries[2].placement = LayoutPlacementReport::At { offset: 16 };
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Envelope",
                    &wrong_outer,
                    &nested,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        let mut wrong_nested = nested.clone();
        wrong_nested.cases[2].payload_fields[0].offset += 1;
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Envelope",
                    &outer,
                    &wrong_nested,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        let mut wrong_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
            unreachable!("fixture is a record")
        };
        fields[1].1 = BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        };
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Envelope",
                    &outer,
                    &nested,
                    &wrong_value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Envelope",
                    &outer,
                    &nested,
                    &value,
                    ByteOrder::BigEndian,
                )
                .is_err()
        );

        let mut short = [0xa5; 19];
        assert!(carrier.apply(&typed, &mut short).is_err());
        assert_eq!(short, [0xa5; 19]);

        let mut corrupted = carrier;
        corrupted.bytes[12] ^= 1;
        let mut unchanged = [0x5a; 20];
        assert!(corrupted.apply(&typed, &mut unchanged).is_err());
        assert_eq!(unchanged, [0x5a; 20]);

        let mut compact_equal = validate_const_materializable_record_with_conventional_sum(
            &typed,
            "Envelope",
            &outer,
            &nested,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("second fixture should validate");
        compact_equal.non_authoritative_layout_report_fingerprint =
            normalized_layout_plan_report_fingerprint(&wrong_outer);
        let error = compact_equal
            .replay_against(
                &typed,
                "Envelope",
                &wrong_outer,
                &nested,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect_err("compact-equal outer substitution must reject");
        assert!(error.0.contains("outer layout drifted"));
    }

    #[test]
    fn multiple_direct_sums_retain_complete_ordered_occurrences_and_reject_row_drift() {
        let typed = typed();
        let choice = conventional_sum_layout(&typed, "Choice");
        let outer = outer_layout(&typed, "TwoChoices", &[0, 12], 24, 4);
        let rows = direct_sum_rows(
            &outer,
            vec![("first", choice.clone()), ("second", choice.clone())],
        );
        let value = BuildTimeValue::Struct {
            type_name: "TwoChoices".into(),
            fields: vec![
                ("first".into(), small_choice_value(0x1122)),
                ("second".into(), choice_value()),
            ],
        };
        let carrier = validate_const_materializable_record_with_conventional_sums(
            &typed,
            "TwoChoices",
            &outer,
            &rows,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("two occurrences of the same sum type should materialize independently");
        assert_eq!(
            carrier
                .nested_sums()
                .iter()
                .map(|row| (row.field(), row.nested_sum().selected_case_ordinal()))
                .collect::<Vec<_>>(),
            [("first", 1), ("second", 2)]
        );
        assert_eq!(
            carrier.bytes(),
            &[
                1, 0, 0, 0, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 9, 0,
                0, 0,
            ]
        );
        carrier
            .replay_against_sum_fields(
                &typed,
                "TwoChoices",
                &outer,
                &rows,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect("the complete field occurrence set should replay");

        let mut missing = rows.clone();
        missing.pop();
        let mut extra = rows.clone();
        extra.push(rows[0].clone());
        let mut reordered = rows.clone();
        reordered.swap(0, 1);
        let duplicate = vec![rows[0].clone(), rows[0].clone()];
        let mut wrong_field_identity = rows.clone();
        wrong_field_identity[1].member_identity = Some(99);
        for (name, changed) in [
            ("missing", missing),
            ("extra", extra),
            ("reordered", reordered),
            ("duplicate", duplicate),
            ("field identity", wrong_field_identity),
        ] {
            assert!(
                carrier
                    .replay_against_sum_fields(
                        &typed,
                        "TwoChoices",
                        &outer,
                        &changed,
                        &value,
                        ByteOrder::LittleEndian,
                    )
                    .is_err(),
                "{name} field rows must reject"
            );
        }

        let mut wrong_layout = rows.clone();
        wrong_layout[1].layout.size += 4;
        let mut wrong_case = rows.clone();
        wrong_case[1].layout.cases[2].ordinal = 1;
        let mut wrong_offset = rows.clone();
        wrong_offset[1].layout.cases[2].payload_fields[0].offset += 1;
        for (name, changed) in [
            ("layout", wrong_layout),
            ("case", wrong_case),
            ("offset", wrong_offset),
        ] {
            assert!(
                carrier
                    .replay_against_sum_fields(
                        &typed,
                        "TwoChoices",
                        &outer,
                        &changed,
                        &value,
                        ByteOrder::LittleEndian,
                    )
                    .is_err(),
                "per-field {name} drift must reject"
            );
        }
        assert!(
            carrier
                .replay_against_sum_fields(
                    &typed,
                    "TwoChoices",
                    &outer,
                    &rows,
                    &value,
                    ByteOrder::BigEndian,
                )
                .is_err()
        );

        let mut short = [0xa5; 23];
        assert!(carrier.apply(&typed, &mut short).is_err());
        assert_eq!(short, [0xa5; 23]);
    }

    #[test]
    fn one_sum_array_retains_each_index_and_atomically_materializes_different_cases() {
        let typed = typed();
        let choice = conventional_sum_layout(&typed, "Choice");
        let outer = outer_layout(&typed, "ChoiceArray", &[0], 24, 4);
        let row = sum_array_row(&outer, "choices", 2, choice);
        let value = BuildTimeValue::Struct {
            type_name: "ChoiceArray".into(),
            fields: vec![(
                "choices".into(),
                BuildTimeValue::Array(vec![small_choice_value(0x1122), choice_value()]),
            )],
        };
        let carrier = validate_const_materializable_record_with_conventional_sum_array(
            &typed,
            "ChoiceArray",
            &outer,
            &row,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the sole direct fixed array should retain each selected sum independently");
        assert_eq!(
            carrier
                .elements()
                .iter()
                .map(|element| (
                    element.literal_index(),
                    element.nested_sum().selected_case_ordinal()
                ))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 2)]
        );
        assert_eq!(
            carrier.bytes(),
            &[
                1, 0, 0, 0, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 9, 0,
                0, 0,
            ]
        );
        carrier
            .replay_against(
                &typed,
                "ChoiceArray",
                &outer,
                &row,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect("the compact layout and every indexed selection should replay");
        let mut destination = [0xa5; 28];
        carrier
            .apply(&typed, &mut destination)
            .expect("the complete outer image should copy atomically");
        assert_eq!(&destination[..24], carrier.bytes());
        assert_eq!(&destination[24..], &[0xa5; 4]);

        let big = validate_const_materializable_record_with_conventional_sum_array(
            &typed,
            "ChoiceArray",
            &outer,
            &row,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("indexed staging retains explicit target byte order");
        assert_eq!(
            big.bytes(),
            &[
                0, 0, 0, 1, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0x11, 0x22, 0x33, 0x44, 9, 0,
                0, 0,
            ]
        );
        assert_ne!(
            carrier.non_authoritative_materialization_report_fingerprint(),
            big.non_authoritative_materialization_report_fingerprint()
        );

        let mut wrong_count = row.clone();
        wrong_count.element_count = 1;
        let mut wrong_stride = row.clone();
        wrong_stride.element_stride += 4;
        let mut wrong_field = row.clone();
        wrong_field.field = "other".into();
        let mut wrong_layout = row.clone();
        wrong_layout.element_layout.cases[2].payload_fields[0].offset += 1;
        for (name, changed) in [
            ("count", wrong_count),
            ("stride", wrong_stride),
            ("field", wrong_field),
            ("layout", wrong_layout),
        ] {
            assert!(
                carrier
                    .replay_against(
                        &typed,
                        "ChoiceArray",
                        &outer,
                        &changed,
                        &value,
                        ByteOrder::LittleEndian,
                    )
                    .is_err(),
                "{name} drift must reject"
            );
        }

        let mut short = [0x5a; 23];
        assert!(carrier.apply(&typed, &mut short).is_err());
        assert_eq!(short, [0x5a; 23]);
        let mut corrupted = carrier;
        corrupted.elements[1].literal_index = 0;
        let mut unchanged = [0x3c; 24];
        assert!(corrupted.apply(&typed, &mut unchanged).is_err());
        assert_eq!(unchanged, [0x3c; 24]);
    }

    #[test]
    fn zero_multiple_nested_recursive_and_mixed_sum_shapes_remain_fenced() {
        let typed = typed();
        let nested = conventional_sum_layout(&typed, "Choice");

        let cases = [(
            "DeepEnvelope",
            BuildTimeValue::Struct {
                type_name: "DeepEnvelope".into(),
                fields: vec![(
                    "inner".into(),
                    BuildTimeValue::Struct {
                        type_name: "InnerEnvelope".into(),
                        fields: vec![("choice".into(), choice_value())],
                    },
                )],
            },
            vec![0],
            12,
            "sum",
        )];
        for (schema, value, offsets, size, expected) in cases {
            let layout = outer_layout(&typed, schema, &offsets, size, 4);
            let error = validate_const_materializable_record_with_conventional_sum(
                &typed,
                schema,
                &layout,
                &nested,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect_err("broader nested-sum shape must reject");
            assert!(error.0.contains(expected), "{schema}: {error:?}");
        }

        for (schema, array_field, value, offsets, size) in [
            (
                "ZeroChoiceArray",
                "choices",
                BuildTimeValue::Struct {
                    type_name: "ZeroChoiceArray".into(),
                    fields: vec![("choices".into(), BuildTimeValue::Array(Vec::new()))],
                },
                vec![0],
                0,
            ),
            (
                "TwoChoiceArrays",
                "first",
                BuildTimeValue::Struct {
                    type_name: "TwoChoiceArrays".into(),
                    fields: vec![
                        ("first".into(), BuildTimeValue::Array(vec![choice_value()])),
                        ("second".into(), BuildTimeValue::Array(vec![choice_value()])),
                    ],
                },
                vec![0, 12],
                24,
            ),
            (
                "NestedChoiceArray",
                "choices",
                BuildTimeValue::Struct {
                    type_name: "NestedChoiceArray".into(),
                    fields: vec![(
                        "choices".into(),
                        BuildTimeValue::Array(vec![BuildTimeValue::Array(vec![
                            choice_value(),
                            choice_value(),
                        ])]),
                    )],
                },
                vec![0],
                24,
            ),
        ] {
            let outer = outer_layout(&typed, schema, &offsets, size, 4);
            let row = sum_array_row(&outer, array_field, 1, nested.clone());
            assert!(
                validate_const_materializable_record_with_conventional_sum_array(
                    &typed,
                    schema,
                    &outer,
                    &row,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err(),
                "{schema} must remain outside the first sum-array rung"
            );
        }

        let mixed_data = unique_data_by_name(&typed, "MixedEnvelope").unwrap();
        let mixed_layout = LayoutPlanReport {
            schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, mixed_data),
            entries: vec![LayoutFieldEntryReport {
                field: "choice".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(8),
            align: 4,
        };
        let error = validate_const_materializable_record_with_conventional_sum(
            &typed,
            "MixedEnvelope",
            &mixed_layout,
            &nested,
            &BuildTimeValue::Struct {
                type_name: "MixedEnvelope".into(),
                fields: vec![(
                    "choice".into(),
                    BuildTimeValue::Case {
                        variant: "Empty".into(),
                        payload: Vec::new(),
                    },
                )],
            },
            ByteOrder::LittleEndian,
        )
        .expect_err("mixed common-field/case shape remains fenced");
        assert!(error.0.contains("mixed common-field/case"), "{error:?}");
    }

    #[test]
    fn nan_reference_text_dynamic_quotient_and_generic_shapes_remain_fenced() {
        let typed = typed();

        let floating_nested = conventional_sum_layout(&typed, "FloatingChoice");
        let floating_outer = outer_layout(&typed, "FloatingEnvelope", &[0], 16, 8);
        let floating_error = validate_const_materializable_record_with_conventional_sum(
            &typed,
            "FloatingEnvelope",
            &floating_outer,
            &floating_nested,
            &BuildTimeValue::Struct {
                type_name: "FloatingEnvelope".into(),
                fields: vec![(
                    "choice".into(),
                    BuildTimeValue::Case {
                        variant: "Number".into(),
                        payload: vec![("value".into(), BuildTimeValue::Float(f64::NAN))],
                    },
                )],
            },
            ByteOrder::LittleEndian,
        )
        .expect_err("NaN remains fenced");
        assert!(floating_error.0.contains("exact raw-NaN"));

        let nested = conventional_sum_layout(&typed, "Choice");
        let unsupported = [
            (
                "BorrowedEnvelope",
                BuildTimeValue::Int(1),
                "reference",
                24,
                8,
            ),
            (
                "TextEnvelope",
                BuildTimeValue::Text(vec![1, 2, 3]),
                "Text",
                24,
                4,
            ),
            ("DynamicEnvelope", BuildTimeValue::Int(1), "dynamic", 24, 8),
            (
                "QuotientEnvelope",
                BuildTimeValue::Case {
                    variant: "Unit".into(),
                    payload: Vec::new(),
                },
                "quotient",
                16,
                4,
            ),
        ];
        for (schema, unsupported_value, expected, size, align) in unsupported {
            let layout = outer_layout(&typed, schema, &[0, 12], size, align);
            let value = BuildTimeValue::Struct {
                type_name: schema.into(),
                fields: vec![
                    ("choice".into(), choice_value()),
                    (
                        match schema {
                            "BorrowedEnvelope" => "borrowed",
                            "TextEnvelope" => "text",
                            "DynamicEnvelope" => "shape",
                            "QuotientEnvelope" => "quotient",
                            _ => unreachable!(),
                        }
                        .into(),
                        unsupported_value,
                    ),
                ],
            };
            let error = validate_const_materializable_record_with_conventional_sum(
                &typed,
                schema,
                &layout,
                &nested,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect_err("unsupported leaf must remain fenced");
            assert!(error.0.contains(expected), "{schema}: {error:?}");
        }

        let generic_data = unique_data_by_name(&typed, "GenericEnvelope").unwrap();
        let generic_layout = LayoutPlanReport {
            schema_report_fingerprint: normalized_schema_report_fingerprint(&typed, generic_data),
            entries: Vec::new(),
            offsets: Some(Vec::new()),
            size: Some(0),
            align: 1,
        };
        let error = validate_const_materializable_record_with_conventional_sum(
            &typed,
            "GenericEnvelope",
            &generic_layout,
            &nested,
            &BuildTimeValue::Struct {
                type_name: "GenericEnvelope".into(),
                fields: vec![
                    ("choice".into(), choice_value()),
                    ("value".into(), BuildTimeValue::Int(1)),
                ],
            },
            ByteOrder::LittleEndian,
        )
        .expect_err("generic owner remains fenced");
        assert!(error.0.contains("generic, opaque, quotient"));
    }
}
