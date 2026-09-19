//! Deriving the bytes of records with sums and sum arrays.

use crate::layouts::layout_plans::const_materializable::{
    unique_data_by_name, validate_value, value_kind,
};
use crate::layouts::layout_plans::const_record_with_nested_sum_materializable::SumReachability;
use crate::layouts::layout_plans::const_record_with_sum_materializable::outer_layouts::{
    exact_named_data, field_occurrence_matches, flatten_literal_array_elements,
    validate_outer_layout, validate_outer_record_owner,
};
use crate::layouts::layout_plans::const_record_with_sum_materializable::{
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization,
};
use crate::layouts::layout_plans::{
    BuildTimeValue, RepeatedFieldInfo, encode_typed_owned_value, exact_struct_fields,
    normalized_schema_report_fingerprint, reflected_field_layout,
    validate_const_materializable_conventional_sum,
};
use layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumFieldLayoutReport, LayoutPlanReport, MaterializationDiagnostic,
    materialize_aggregate_layout_into,
};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

pub(crate) struct DerivedRecordWithSumMaterialization {
    pub(crate) schema_report_fingerprint: u64,
    pub(crate) nested_sums: Vec<ValidatedConstRecordSumFieldMaterialization>,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct DerivedRecordWithSumArrayMaterialization {
    pub(crate) schema_report_fingerprint: u64,
    pub(crate) elements: Vec<ValidatedConstRecordSumArrayElementMaterialization>,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct DerivedRecordWithSumArraysMaterialization {
    pub(crate) schema_report_fingerprint: u64,
    pub(crate) arrays: Vec<ValidatedConstRecordSumArrayFieldMaterialization>,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct PreparedSumArrayField {
    pub(crate) field: String,
    pub(crate) field_identity: Option<u64>,
    pub(crate) elements: Vec<ValidatedConstRecordSumArrayElementSelection>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) size: u64,
    pub(crate) align: u64,
    pub(crate) repeated: RepeatedFieldInfo,
}

/// Validate one direct fixed-array-of-sums field's compact row and every
/// literal element's selected case against the retained element layout,
/// staging the field's complete repeated bytes once. `element_hops` carries
/// every consecutive literal arity, outermost first — `[[S; 2]; 3]` spells
/// `[3, 2]` and stages six packed elements. The same preparation runs
/// wherever the field sits — the standalone sum-array rung or a record level
/// inside the recursive record/sum report.
pub(crate) fn prepare_sum_array_field(
    typed: &TypedTrees,
    array_field: &typed_trees::data::DataField,
    sum_data: &DataDefinition,
    element_hops: &[usize],
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    array_value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<PreparedSumArrayField, MaterializationDiagnostic> {
    if !field_occurrence_matches(
        &array_layout.field,
        array_layout.member_identity,
        array_field.name.as_str(),
        array_field.identity,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable compact sum-array row for `{}` is missing, duplicated, or out of authored field order",
            array_field.name
        )));
    }
    let element_count = element_hops
        .iter()
        .try_fold(1usize, |count, hop| count.checked_mul(*hop))
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable sum-array count exceeds canonical report width".into(),
            )
        })?;
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
    let element_values =
        flatten_literal_array_elements(array_field.name.as_str(), element_hops, array_value)?;
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
    for (index, element_value) in element_values.into_iter().enumerate() {
        let nested_sum = validate_const_materializable_conventional_sum(
            typed,
            sum_data.name.as_str(),
            &array_layout.element_layout,
            element_value,
            byte_order,
        )?;
        array_bytes.extend_from_slice(nested_sum.bytes());
        let (
            non_authoritative_schema_report_fingerprint,
            value,
            non_authoritative_layout_report_fingerprint,
            selected_case_identity,
            selected_case_ordinal,
            bytes,
            non_authoritative_materialization_report_fingerprint,
        ) = nested_sum.into_compact_selection();
        elements.push(ValidatedConstRecordSumArrayElementSelection {
            literal_index: u64::try_from(index).map_err(|_| {
                MaterializationDiagnostic(
                    "ConstMaterializable sum-array index exceeds canonical width".into(),
                )
            })?,
            non_authoritative_schema_report_fingerprint,
            value,
            non_authoritative_layout_report_fingerprint,
            selected_case_identity,
            selected_case_ordinal,
            bytes,
            non_authoritative_materialization_report_fingerprint,
        });
    }
    if array_bytes.len() != total_size_usize {
        return Err(MaterializationDiagnostic(format!(
            "value.{} encoded to {} bytes, expected {total_size}",
            array_field.name,
            array_bytes.len()
        )));
    }
    Ok(PreparedSumArrayField {
        field: array_field.name.to_string(),
        field_identity: array_field.identity,
        elements,
        bytes: array_bytes,
        size: total_size,
        align: array_layout.element_layout.align,
        repeated: RepeatedFieldInfo {
            element_size: array_layout.element_stride,
            element_align: array_layout.element_layout.align,
            element_count: array_layout.element_count,
        },
    })
}

pub(crate) struct EncodedOuterField {
    pub(crate) name: String,
    pub(crate) identity: Option<u64>,
    pub(crate) size: u64,
    pub(crate) align: u64,
    pub(crate) repeated: Option<RepeatedFieldInfo>,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn derive_record_with_sum_bytes(
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
            DataShapeKind::Enum | DataShapeKind::Mixed => {
                if field.relevance.is_erased() {
                    continue;
                }
                direct_sums.push((field, named));
            }
            DataShapeKind::Empty | DataShapeKind::Record => {}
        }
    }
    if direct_sums.is_empty() {
        return Err(
        MaterializationDiagnostic(
            "nested-sum ConstMaterializable requires at least one direct runtime-relevant case-bearing field"
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
    materialize_aggregate_layout_into(layout, &schemas, &values, byte_order, &mut bytes)?;

    Ok(DerivedRecordWithSumMaterialization {
        schema_report_fingerprint,
        nested_sums,
        bytes,
    })
}

pub(crate) fn derive_record_with_sum_array_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedRecordWithSumArrayMaterialization, MaterializationDiagnostic> {
    let mut derived = derive_record_with_sum_arrays_bytes(
        typed,
        schema_name,
        layout,
        std::slice::from_ref(array_layout),
        value,
        byte_order,
    )?;
    if derived.arrays.len() != 1 {
        return Err(MaterializationDiagnostic(format!(
            "singular ConstMaterializable sum-array validation requires exactly one qualifying occurrence; found {}",
            derived.arrays.len()
        )));
    }
    let array = derived.arrays.pop().expect("exactly one array occurrence");
    let elements = reconstruct_legacy_sum_array_elements(
        typed,
        schema_name,
        array_layout,
        &array,
        byte_order,
    )?;
    Ok(DerivedRecordWithSumArrayMaterialization {
        schema_report_fingerprint: derived.schema_report_fingerprint,
        elements,
        bytes: derived.bytes,
    })
}

fn reconstruct_legacy_sum_array_elements(
    typed: &TypedTrees,
    schema_name: &str,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    compact: &ValidatedConstRecordSumArrayFieldMaterialization,
    byte_order: ByteOrder,
) -> Result<Vec<ValidatedConstRecordSumArrayElementMaterialization>, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    let mut matching_fields = typed.data_members(data).iter().filter_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        field_occurrence_matches(
            field.name.as_str(),
            field.identity,
            compact.field(),
            compact.field_identity(),
        )
        .then_some(field)
    });
    let field = matching_fields.next().ok_or_else(|| {
        MaterializationDiagnostic(
            "singular ConstMaterializable sum-array field disappeared after plural replay".into(),
        )
    })?;
    if matching_fields.next().is_some() {
        return Err(MaterializationDiagnostic(
            "singular ConstMaterializable sum-array field identity is ambiguous".into(),
        ));
    }
    let TypeReferenceNode::FixedArray { element_type, .. } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return Err(MaterializationDiagnostic(
            "singular ConstMaterializable sum-array field lost its fixed-array type".into(),
        ));
    };
    let sum_data = exact_named_data(typed, *element_type)?.ok_or_else(|| {
        MaterializationDiagnostic(
            "singular ConstMaterializable sum-array element lost its exact nominal identity".into(),
        )
    })?;
    let mut elements = Vec::new();
    elements
        .try_reserve_exact(compact.elements.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "singular ConstMaterializable legacy element view exceeds compiler resources"
                    .into(),
            )
        })?;
    for selection in &compact.elements {
        let nested_sum = validate_const_materializable_conventional_sum(
            typed,
            sum_data.name.as_str(),
            &array_layout.element_layout,
            selection.value(),
            byte_order,
        )?;
        if selection.non_authoritative_schema_report_fingerprint()
            != nested_sum.non_authoritative_schema_report_fingerprint()
            || selection.non_authoritative_layout_report_fingerprint()
                != nested_sum.non_authoritative_layout_report_fingerprint()
            || selection.selected_case_identity() != nested_sum.selected_case_identity()
            || selection.selected_case_ordinal() != nested_sum.selected_case_ordinal()
            || selection.bytes() != nested_sum.bytes()
            || selection.non_authoritative_materialization_report_fingerprint()
                != nested_sum.non_authoritative_materialization_report_fingerprint()
        {
            return Err(MaterializationDiagnostic(
                "singular ConstMaterializable legacy element view drifted from compact replay"
                    .into(),
            ));
        }
        elements.push(ValidatedConstRecordSumArrayElementMaterialization {
            literal_index: selection.literal_index(),
            nested_sum,
        });
    }
    Ok(elements)
}

pub(crate) fn derive_record_with_sum_arrays_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    array_layouts: &[ConventionalSumArrayFieldLayoutReport],
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedRecordWithSumArraysMaterialization, MaterializationDiagnostic> {
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

    let mut selected_arrays = Vec::new();
    selected_arrays
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable sum-array occurrence set exceeds compiler resources".into(),
            )
        })?;
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
        let mut selected_direct_array = false;
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
                        DataShapeKind::Enum | DataShapeKind::Mixed => {
                            if *length == 0 {
                                return Err(MaterializationDiagnostic(format!(
                                    "sum-array field `{}` must have nonzero literal length",
                                    field.name
                                )));
                            }
                            selected_arrays.push((field, named, *length));
                            selected_direct_array = true;
                        }
                        DataShapeKind::Empty | DataShapeKind::Record => {}
                    }
                }
            }
            _ => {
                if let Some(named) = exact_named_data(typed, field.type_reference)?
                    && matches!(
                        DataDefinition::shape_kind_from_members(typed.data_members(named)),
                        DataShapeKind::Enum | DataShapeKind::Mixed
                    )
                {
                    return Err(MaterializationDiagnostic(
                        "ConstMaterializable sum-array rung does not combine direct sum fields with its array field"
                            .into(),
                    ));
                }
            }
        }
        if !selected_direct_array && reachability.type_contains_sum(field.type_reference)? {
            return Err(MaterializationDiagnostic(format!(
                "field `{}` reaches a sum through a nested array or record, outside the direct sum-array rung",
                field.name
            )));
        }
    }
    if selected_arrays.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable sum-array rung requires a nonempty direct nonzero literal fixed-array-of-sums field set"
                .into(),
        ));
    }
    if array_layouts.len() != selected_arrays.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable sum-array rows contain {} occurrence(s), expected the complete authored-order set of {}",
            array_layouts.len(),
            selected_arrays.len()
        )));
    }
    let mut prepared_arrays = Vec::new();
    prepared_arrays
        .try_reserve_exact(selected_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable sum-array staging set exceeds compiler resources".into(),
            )
        })?;
    for ((array_field, sum_data, element_count), array_layout) in
        selected_arrays.into_iter().zip(array_layouts)
    {
        let array_value = supplied
            .get(array_field.name.as_str())
            .expect("complete record value checked above");
        prepared_arrays.push(prepare_sum_array_field(
            typed,
            array_field,
            sum_data,
            std::slice::from_ref(&element_count),
            array_layout,
            array_value,
            byte_order,
        )?);
    }

    let mut encoded_fields = Vec::new();
    let mut active = vec![data.symbol];
    let mut prepared_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete record value checked above");
        if prepared_arrays.get(prepared_index).is_some_and(|prepared| {
            field_occurrence_matches(
                field.name.as_str(),
                field.identity,
                &prepared.field,
                prepared.field_identity,
            )
        }) {
            let prepared = &mut prepared_arrays[prepared_index];
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: prepared.size,
                align: prepared.align,
                repeated: Some(prepared.repeated),
                bytes: std::mem::take(&mut prepared.bytes),
            });
            prepared_index += 1;
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
    if prepared_index != prepared_arrays.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable sum-array staging did not consume the complete authored-order set"
                .into(),
        ));
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
    materialize_aggregate_layout_into(layout, &schemas, &values, byte_order, &mut bytes)?;

    let mut arrays = Vec::new();
    arrays
        .try_reserve_exact(prepared_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable retained sum-array occurrence set exceeds compiler resources"
                    .into(),
            )
        })?;
    for prepared in prepared_arrays {
        arrays.push(ValidatedConstRecordSumArrayFieldMaterialization {
            field: prepared.field,
            field_identity: prepared.field_identity,
            elements: prepared.elements,
        });
    }
    Ok(DerivedRecordWithSumArraysMaterialization {
        schema_report_fingerprint,
        arrays,
        bytes,
    })
}
