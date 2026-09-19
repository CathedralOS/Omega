//! Recursive nested-record byte derivation and staging.

use super::{
    AggregateFieldSchema, AggregateFieldValue, BuildTimeValue, ByteOrder,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalRecordArrayFieldLayoutReport,
    ConventionalRecordSumPathsLayoutReport, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumFieldLayoutReport, DataDefinition, DataMember, DataShapeKind,
    DerivedNestedRecordSumMaterialization, DerivedNestedRecordSumsMaterialization,
    DerivedRecordLevelMaterialization, DerivedRecursiveNestedSumsMaterialization,
    EncodedOuterField, MaterializationDiagnostic, NestedPathsView, PreparedSumArrayField,
    RepeatedFieldInfo, SumReachability, TypedTrees,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordArrayElementSelection, ValidatedConstRecordArrayFieldMaterialization,
    ValidatedConstRecordSumArrayFieldMaterialization, ValidatedConstRecordSumFieldMaterialization,
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization, encode_typed_owned_value,
    exact_named_data, exact_struct_fields, field_occurrence_matches,
    flatten_literal_array_elements, materialize_aggregate_layout_into,
    normalized_schema_report_fingerprint, prepare_sum_array_field, record_sum_profile, recursive,
    reflected_field_layout, reject_sum_array_type, unique_data_by_name,
    validate_const_materializable_conventional_sum,
    validate_const_materializable_record_with_conventional_sums, validate_outer_layout,
    validate_outer_record_owner, validate_value, value_kind,
};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

/// One record level's runtime-relevant direct-sum children classified under
/// the general recursive rule, in authored member order.
pub(super) struct RecordLevelChildren<'a> {
    /// Direct conventional pure-sum fields.
    pub(super) direct_sums: Vec<(&'a typed_trees::data::DataField, &'a DataDefinition)>,
    /// Direct nonzero literal fixed arrays of conventional pure sums. The
    /// `Vec<usize>` is every consecutive literal element arity, outermost
    /// first — `[[S; 2]; 3]` carries `[3, 2]` and its packed row's element
    /// count is the product.
    pub(super) direct_sum_arrays: Vec<(
        &'a typed_trees::data::DataField,
        &'a DataDefinition,
        Vec<usize>,
    )>,
    /// Direct nonzero literal fixed arrays of records still reaching sums
    /// inside the element, under the same flattened-hop rule.
    pub(super) direct_record_arrays: Vec<(
        &'a typed_trees::data::DataField,
        &'a DataDefinition,
        Vec<usize>,
    )>,
    /// Record fields whose exact type still reaches sums below this level.
    pub(super) record_paths: Vec<(&'a typed_trees::data::DataField, &'a DataDefinition)>,
}

/// Classify every runtime-relevant field of one record level exactly as the
/// projection does: direct pure sums, direct literal fixed arrays of pure
/// sums, direct literal fixed arrays of records still reaching sums inside
/// the element, and record fields still reaching sums below. Consecutive
/// literal element hops flatten into the packed row's element count — a
/// `[[S; 2]; 3]` field is six packed sums — while mixed elements,
/// non-literal lengths, and zero-length hops remain fenced, and every
/// declared member must be supplied by the value.
pub(super) fn classify_record_level_children<'a>(
    typed: &'a TypedTrees,
    data: &'a DataDefinition,
    supplied: &std::collections::BTreeMap<&str, &BuildTimeValue>,
    reachability: &mut SumReachability<'_>,
) -> Result<RecordLevelChildren<'a>, MaterializationDiagnostic> {
    let members = typed.data_members(data);
    let mut children = RecordLevelChildren {
        direct_sums: Vec::new(),
        direct_sum_arrays: Vec::new(),
        direct_record_arrays: Vec::new(),
        record_paths: Vec::new(),
    };
    children
        .direct_sums
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level direct-sum set exceeds compiler resources".into(),
            )
        })?;
    children
        .direct_sum_arrays
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level sum-array set exceeds compiler resources".into(),
            )
        })?;
    children
        .direct_record_arrays
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level record-array set exceeds compiler resources"
                    .into(),
            )
        })?;
    children
        .record_paths
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level path set exceeds compiler resources".into(),
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
        if !reachability.type_contains_sum(field.type_reference)? {
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
                // Consecutive literal element hops pack contiguously —
                // `[[S; N]; M]` holds `M * N` elements — so the level spells
                // the whole hop list beside the innermost element's report.
                let mut hops = vec![*length];
                let mut element_reference = *element_type;
                loop {
                    match typed.type_reference_table.type_reference(element_reference) {
                        TypeReferenceNode::FixedArray {
                            element_type: nested_element,
                            length: FixedArrayLength::Literal(nested_length),
                        } => {
                            hops.push(*nested_length);
                            element_reference = *nested_element;
                        }
                        TypeReferenceNode::FixedArray { .. } => {
                            return Err(MaterializationDiagnostic(format!(
                                "value.{} reaches a sum through a non-literal-length array",
                                field.name
                            )));
                        }
                        _ => break,
                    }
                }
                let Some(named) = exact_named_data(typed, element_reference)? else {
                    return Err(MaterializationDiagnostic(format!(
                        "value.{} reaches a sum through an array element without one exact nominal identity",
                        field.name
                    )));
                };
                if hops.contains(&0) {
                    return Err(MaterializationDiagnostic(format!(
                        "value.{} must have nonzero literal length",
                        field.name
                    )));
                }
                match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
                    // A mixed element is one case-bearing interior with
                    // common fields beside the overlay: the same compact
                    // element row carries it.
                    DataShapeKind::Enum | DataShapeKind::Mixed => {
                        children.direct_sum_arrays.push((field, named, hops));
                    }
                    // A record element crosses its own record boundary
                    // inside each index — the same literal element hop the
                    // projection admits, carried by the recursive report the
                    // row retains once for every element.
                    DataShapeKind::Record => {
                        children.direct_record_arrays.push((field, named, hops));
                    }
                    DataShapeKind::Empty => {
                        return Err(MaterializationDiagnostic(format!(
                            "value.{} reaches a sum through an array deeper than one literal element hop",
                            field.name
                        )));
                    }
                }
            }
            TypeReferenceNode::FixedArray { .. } => {
                return Err(MaterializationDiagnostic(format!(
                    "value.{} reaches a sum through a non-literal-length array",
                    field.name
                )));
            }
            _ => {
                let Some(named) = exact_named_data(typed, field.type_reference)? else {
                    return Err(MaterializationDiagnostic(format!(
                        "field `{}` lacks one exact nominal identity",
                        field.name
                    )));
                };
                match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
                    // A direct case-bearing field — pure sum or mixed —
                    // coexists with the level's deeper record paths: it takes
                    // the same per-field custody the leaf level retains
                    // through `child_sum_layouts`.
                    DataShapeKind::Enum | DataShapeKind::Mixed => {
                        children.direct_sums.push((field, named));
                    }
                    DataShapeKind::Record => {
                        validate_outer_record_owner(typed, named)?;
                        children.record_paths.push((field, named));
                    }
                    DataShapeKind::Empty => {
                        return Err(MaterializationDiagnostic(format!(
                            "field `{}` does not name the required inner record",
                            field.name
                        )));
                    }
                }
            }
        }
    }
    Ok(children)
}

/// Validate one level's direct-sum rows against its classified fields.
pub(super) fn validate_direct_sum_rows(
    typed: &TypedTrees,
    direct_sums: &[(&typed_trees::data::DataField, &DataDefinition)],
    rows: &[ConventionalSumFieldLayoutReport],
    supplied: &std::collections::BTreeMap<&str, &BuildTimeValue>,
    byte_order: ByteOrder,
    role: &str,
) -> Result<Vec<ValidatedConstRecordSumFieldMaterialization>, MaterializationDiagnostic> {
    if rows.len() != direct_sums.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable {role} report contains {} direct-sum row(s), expected the complete authored-order set of {}",
            rows.len(),
            direct_sums.len()
        )));
    }
    let mut nested_sums = Vec::new();
    nested_sums
        .try_reserve_exact(direct_sums.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level direct-sum custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for ((field, sum_data), row) in direct_sums.iter().zip(rows) {
        if !field_occurrence_matches(
            &row.field,
            row.member_identity,
            field.name.as_str(),
            field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable {role} direct-sum row for `{}` is missing, duplicated, or out of authored field order",
                field.name
            )));
        }
        let sum_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
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
    Ok(nested_sums)
}

/// Validate one level's compact sum-array rows against its classified fields,
/// staging each field's complete repeated bytes and per-element custody.
pub(super) fn validate_direct_sum_array_rows(
    typed: &TypedTrees,
    direct_sum_arrays: &[(&typed_trees::data::DataField, &DataDefinition, Vec<usize>)],
    rows: &[ConventionalSumArrayFieldLayoutReport],
    supplied: &std::collections::BTreeMap<&str, &BuildTimeValue>,
    byte_order: ByteOrder,
    role: &str,
) -> Result<Vec<PreparedSumArrayField>, MaterializationDiagnostic> {
    if rows.len() != direct_sum_arrays.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable {role} report contains {} sum-array row(s), expected the complete authored-order set of {}",
            rows.len(),
            direct_sum_arrays.len()
        )));
    }
    let mut prepared_arrays = Vec::new();
    prepared_arrays
        .try_reserve_exact(direct_sum_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level sum-array staging set exceeds compiler resources"
                    .into(),
            )
        })?;
    for ((array_field, sum_data, element_hops), array_layout) in direct_sum_arrays.iter().zip(rows)
    {
        let array_value = supplied
            .get(array_field.name.as_str())
            .expect("complete outer value checked above");
        prepared_arrays.push(prepare_sum_array_field(
            typed,
            array_field,
            sum_data,
            element_hops,
            array_layout,
            array_value,
            byte_order,
        )?);
    }
    Ok(prepared_arrays)
}

/// One direct `[R; N]` field's staged repeated bytes and per-element
/// recursive custody, prepared before the level's complete image exists.
pub(super) struct PreparedRecordArrayField {
    pub(super) field: String,
    pub(super) field_identity: Option<u64>,
    pub(super) elements: Vec<ValidatedConstRecordArrayElementSelection>,
    pub(super) bytes: Vec<u8>,
    pub(super) size: u64,
    pub(super) align: u64,
    pub(super) repeated: RepeatedFieldInfo,
}

/// Validate one direct `[R; N]` field's compact row and every literal
/// element's recursive record/sum value against the row's shared element
/// report, staging the field's complete repeated bytes once. Each element
/// keeps its literal index beside its own recursive materialization
/// coordinate; the complete element report lives once in the row. The same
/// preparation runs wherever the field sits — a leaf level or a branch
/// level inside the recursive record/sum report.
pub(super) fn prepare_record_array_field(
    typed: &TypedTrees,
    array_field: &typed_trees::data::DataField,
    element_data: &DataDefinition,
    element_hops: &[usize],
    array_layout: &ConventionalRecordArrayFieldLayoutReport,
    array_value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<PreparedRecordArrayField, MaterializationDiagnostic> {
    if !field_occurrence_matches(
        &array_layout.field,
        array_layout.member_identity,
        array_field.name.as_str(),
        array_field.identity,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable compact record-array row for `{}` is missing, duplicated, or out of authored field order",
            array_field.name
        )));
    }
    let element_count = element_hops
        .iter()
        .try_fold(1usize, |count, hop| count.checked_mul(*hop))
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable record-array count exceeds canonical report width".into(),
            )
        })?;
    let element_count_u64 = u64::try_from(element_count).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable record-array count exceeds canonical report width".into(),
        )
    })?;
    let element_size = array_layout.inner.outer_layout().size.ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "ConstMaterializable compact record-array row for `{}` requires one exact element extent",
            array_field.name
        ))
    })?;
    if array_layout.element_count != element_count_u64
        || array_layout.element_stride != element_size
    {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable compact record-array count/stride drifted for `{}`",
            array_field.name
        )));
    }
    let element_values =
        flatten_literal_array_elements(array_field.name.as_str(), element_hops, array_value)?;
    let mut elements = Vec::new();
    elements.try_reserve_exact(element_count).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable record-array element custody exceeds compiler resources".into(),
        )
    })?;
    let total_size = array_layout
        .element_stride
        .checked_mul(array_layout.element_count)
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable record-array physical extent overflows".into(),
            )
        })?;
    let total_size_usize = usize::try_from(total_size).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable record-array physical extent exceeds compiler host".into(),
        )
    })?;
    let mut array_bytes = Vec::new();
    array_bytes
        .try_reserve_exact(total_size_usize)
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-array staged bytes exceed compiler resources".into(),
            )
        })?;
    for (index, element_value) in element_values.into_iter().enumerate() {
        let inner = recursive::validate_with_reachability(
            typed,
            element_data.name.as_str(),
            &array_layout.inner,
            element_value,
            byte_order,
            reachability,
        )?;
        if u64::try_from(inner.bytes().len()).ok() != Some(element_size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} element {index} encoded to {} bytes, expected {element_size}",
                array_field.name,
                inner.bytes().len()
            )));
        }
        let selection = ValidatedConstRecordArrayElementSelection {
            literal_index: u64::try_from(index).map_err(|_| {
                MaterializationDiagnostic(
                    "ConstMaterializable record-array index exceeds canonical width".into(),
                )
            })?,
            non_authoritative_materialization_report_fingerprint: inner
                .non_authoritative_materialization_report_fingerprint(),
            value: element_value.clone(),
            bytes: inner.bytes().to_vec(),
        };
        array_bytes.extend_from_slice(inner.bytes());
        elements.push(selection);
    }
    if array_bytes.len() != total_size_usize {
        return Err(MaterializationDiagnostic(format!(
            "value.{} encoded to {} bytes, expected {total_size}",
            array_field.name,
            array_bytes.len()
        )));
    }
    Ok(PreparedRecordArrayField {
        field: array_field.name.to_string(),
        field_identity: array_field.identity,
        elements,
        bytes: array_bytes,
        size: total_size,
        align: array_layout.inner.outer_layout().align,
        repeated: RepeatedFieldInfo {
            element_size: array_layout.element_stride,
            element_align: array_layout.inner.outer_layout().align,
            element_count: array_layout.element_count,
        },
    })
}

/// Validate one level's compact record-array rows against its classified
/// fields, staging each field's complete repeated bytes and per-element
/// recursive custody.
pub(super) fn validate_direct_record_array_rows(
    typed: &TypedTrees,
    direct_record_arrays: &[(&typed_trees::data::DataField, &DataDefinition, Vec<usize>)],
    rows: &[ConventionalRecordArrayFieldLayoutReport],
    supplied: &std::collections::BTreeMap<&str, &BuildTimeValue>,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
    role: &str,
) -> Result<Vec<PreparedRecordArrayField>, MaterializationDiagnostic> {
    if rows.len() != direct_record_arrays.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable {role} report contains {} record-array row(s), expected the complete authored-order set of {}",
            rows.len(),
            direct_record_arrays.len()
        )));
    }
    let mut prepared_arrays = Vec::new();
    prepared_arrays
        .try_reserve_exact(direct_record_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level record-array staging set exceeds compiler resources"
                    .into(),
            )
        })?;
    for ((array_field, element_data, element_hops), array_layout) in
        direct_record_arrays.iter().zip(rows)
    {
        let array_value = supplied
            .get(array_field.name.as_str())
            .expect("complete outer value checked above");
        prepared_arrays.push(prepare_record_array_field(
            typed,
            array_field,
            element_data,
            element_hops,
            array_layout,
            array_value,
            byte_order,
            reachability,
        )?);
    }
    Ok(prepared_arrays)
}

/// Stage one record level's runtime fields in authored member order. The
/// caller's `occurrence_for` supplies the next ordered deeper-path custody
/// when `field` opens one; the level's own direct sums, prepared sum
/// arrays, and prepared record arrays join by field identity. Everything
/// else encodes as an opaque aggregate value after `validate_value`.
pub(super) fn stage_record_level_fields(
    typed: &TypedTrees,
    data: &DataDefinition,
    members: &[DataMember],
    supplied: &std::collections::BTreeMap<&str, &BuildTimeValue>,
    mut occurrence_for: impl FnMut(
        &typed_trees::data::DataField,
    ) -> Result<Option<EncodedOuterField>, MaterializationDiagnostic>,
    prepared_arrays: &mut [PreparedSumArrayField],
    prepared_record_arrays: &mut [PreparedRecordArrayField],
    nested_sums: &[ValidatedConstRecordSumFieldMaterialization],
    byte_order: ByteOrder,
    role: &str,
) -> Result<Vec<EncodedOuterField>, MaterializationDiagnostic> {
    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable record-level active path exceeds compiler resources".into(),
        )
    })?;
    active.push(data.symbol);
    let mut prepared_index = 0usize;
    let mut prepared_record_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        if let Some(encoded) = occurrence_for(field)? {
            encoded_fields.push(encoded);
            continue;
        }
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
        if prepared_record_arrays
            .get(prepared_record_index)
            .is_some_and(|prepared| {
                field_occurrence_matches(
                    field.name.as_str(),
                    field.identity,
                    &prepared.field,
                    prepared.field_identity,
                )
            })
        {
            let prepared = &mut prepared_record_arrays[prepared_record_index];
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: prepared.size,
                align: prepared.align,
                repeated: Some(prepared.repeated),
                bytes: std::mem::take(&mut prepared.bytes),
            });
            prepared_record_index += 1;
            continue;
        }
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
    if prepared_index != prepared_arrays.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable {role} staging did not consume the complete authored-order sum-array set"
        )));
    }
    if prepared_record_index != prepared_record_arrays.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable {role} staging did not consume the complete authored-order record-array set"
        )));
    }
    Ok(encoded_fields)
}

/// Stage the complete outer record image from authored-order encoded fields.
pub(super) fn materialize_level_bytes(
    outer_layout: &layout_plans::LayoutPlanReport,
    encoded_fields: Vec<EncodedOuterField>,
    byte_order: ByteOrder,
) -> Result<Vec<u8>, MaterializationDiagnostic> {
    validate_outer_layout(outer_layout, &encoded_fields)?;
    let byte_len =
        usize::try_from(outer_layout.size.expect("validated fixed extent")).map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level outer extent exceeds compiler host".into(),
            )
        })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable record-level staged bytes exceed compiler resources".into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level schema staging exceeds compiler resources".into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level value staging exceeds compiler resources".into(),
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
    materialize_aggregate_layout_into(outer_layout, &schemas, &values, byte_order, &mut bytes)?;
    Ok(bytes)
}

/// Derive one recursive leaf level's complete direct-sum children: every
/// direct conventional pure-sum field, every direct fixed array of pure
/// sums, and every direct fixed array of records still reaching sums beside
/// the level's flat outer plan. A leaf level cannot carry deeper record
/// paths; a value-side record field still reaching sums drifts from the
/// supplied `Leaf` report and rejects.
pub(super) fn derive_record_level_children_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    outer_layout: &layout_plans::LayoutPlanReport,
    child_sum_layouts: &[ConventionalSumFieldLayoutReport],
    child_sum_array_layouts: &[ConventionalSumArrayFieldLayoutReport],
    child_record_array_layouts: &[ConventionalRecordArrayFieldLayoutReport],
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedRecordLevelMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable record-level outer layout schema report fingerprint does not match `{schema_name}`"
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
    let children = classify_record_level_children(typed, data, &supplied, reachability)?;
    if let Some((field, _)) = children.record_paths.first() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable record-level leaf cannot carry the deeper record path through `{}`",
            field.name
        )));
    }
    if children.direct_sums.is_empty()
        && children.direct_sum_arrays.is_empty()
        && children.direct_record_arrays.is_empty()
    {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable record-level requires at least one direct runtime-relevant pure-sum, sum-array, or record-array field"
                .into(),
        ));
    }
    let mut total_leaf_occurrences = child_sum_layouts
        .len()
        .checked_add(child_sum_array_layouts.len())
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level leaf occurrence count overflows".to_owned(),
            )
        })?;
    for row in child_record_array_layouts {
        total_leaf_occurrences = total_leaf_occurrences
            .checked_add(row.inner.leaf_occurrence_count().ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable record-level leaf occurrence count overflows".to_owned(),
                )
            })?)
            .ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable record-level leaf occurrence count overflows".to_owned(),
                )
            })?;
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic("ConstMaterializable record-level record arrays exceed the global leaf occurrence bound".to_owned()));
        }
    }
    let nested_sums = validate_direct_sum_rows(
        typed,
        &children.direct_sums,
        child_sum_layouts,
        &supplied,
        byte_order,
        "record-level",
    )?;
    let mut prepared_arrays = validate_direct_sum_array_rows(
        typed,
        &children.direct_sum_arrays,
        child_sum_array_layouts,
        &supplied,
        byte_order,
        "record-level",
    )?;
    let mut prepared_record_arrays = validate_direct_record_array_rows(
        typed,
        &children.direct_record_arrays,
        child_record_array_layouts,
        &supplied,
        byte_order,
        reachability,
        "record-level",
    )?;
    let encoded_fields = stage_record_level_fields(
        typed,
        data,
        members,
        &supplied,
        |_| Ok(None),
        &mut prepared_arrays,
        &mut prepared_record_arrays,
        &nested_sums,
        byte_order,
        "record-level",
    )?;
    let bytes = materialize_level_bytes(outer_layout, encoded_fields, byte_order)?;
    let mut nested_sum_arrays = Vec::new();
    nested_sum_arrays
        .try_reserve_exact(prepared_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level sum-array custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for prepared in prepared_arrays {
        nested_sum_arrays.push(ValidatedConstRecordSumArrayFieldMaterialization {
            field: prepared.field,
            field_identity: prepared.field_identity,
            elements: prepared.elements,
        });
    }
    let mut nested_record_arrays = Vec::new();
    nested_record_arrays
        .try_reserve_exact(prepared_record_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable record-level record-array custody exceeds compiler resources"
                    .into(),
            )
        })?;
    for prepared in prepared_record_arrays {
        nested_record_arrays.push(ValidatedConstRecordArrayFieldMaterialization {
            field: prepared.field,
            field_identity: prepared.field_identity,
            elements: prepared.elements,
        });
    }
    Ok(DerivedRecordLevelMaterialization {
        schema_report_fingerprint,
        nested_sums,
        nested_sum_arrays,
        nested_record_arrays,
        bytes,
    })
}
pub(super) fn derive_recursive_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedRecursiveNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural recursive outer layout schema report fingerprint does not match `{schema_name}`"
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

    let children = classify_record_level_children(typed, data, &supplied, reachability)?;
    let candidates = children.record_paths;
    if candidates.is_empty() {
        return Err(MaterializationDiagnostic("ConstMaterializable plural recursive paths require a nonempty qualifying occurrence set".to_owned()));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural recursive report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let nested_sums = validate_direct_sum_rows(
        typed,
        &children.direct_sums,
        &path_layout.child_sum_layouts,
        &supplied,
        byte_order,
        "plural recursive",
    )?;
    let mut prepared_arrays = validate_direct_sum_array_rows(
        typed,
        &children.direct_sum_arrays,
        &path_layout.child_sum_array_layouts,
        &supplied,
        byte_order,
        "plural recursive",
    )?;
    let mut prepared_record_arrays = validate_direct_record_array_rows(
        typed,
        &children.direct_record_arrays,
        &path_layout.child_record_array_layouts,
        &supplied,
        byte_order,
        reachability,
        "plural recursive",
    )?;
    let mut total_leaf_occurrences = path_layout
        .child_sum_layouts
        .len()
        .checked_add(path_layout.child_sum_array_layouts.len())
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable plural recursive leaf occurrence count overflows".to_owned(),
            )
        })?;
    for row in &path_layout.child_record_array_layouts {
        total_leaf_occurrences = total_leaf_occurrences
            .checked_add(row.inner.leaf_occurrence_count().ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable plural recursive leaf occurrence count overflows"
                        .to_owned(),
                )
            })?)
            .ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable plural recursive leaf occurrence count overflows"
                        .to_owned(),
                )
            })?;
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic("ConstMaterializable plural recursive record arrays exceed the global leaf occurrence bound".to_owned()));
        }
    }
    for path in &path_layout.paths {
        total_leaf_occurrences = total_leaf_occurrences
            .checked_add(path.inner.leaf_occurrence_count().ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable plural recursive leaf occurrence count overflows"
                        .to_owned(),
                )
            })?)
            .ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable plural recursive leaf occurrence count overflows"
                        .to_owned(),
                )
            })?;
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic("ConstMaterializable plural recursive paths exceed the global leaf occurrence bound".to_owned()));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural recursive inner custody exceeds compiler resources"
                    .to_owned(),
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
                "ConstMaterializable plural recursive path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner = recursive::validate_with_reachability(
            typed,
            inner_data.name.as_str(),
            &path.inner,
            inner_value,
            byte_order,
            reachability,
        )?;
        let inner_size = path.inner.outer_layout().size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural recursive path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural recursive inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(ValidatedConstRecursiveNestedSumOccurrenceMaterialization {
            outer_field: inner_field.name.to_string(),
            outer_member_identity: inner_field.identity,
            inner,
        });
    }

    let mut occurrence_index = 0usize;
    let encoded_fields = stage_record_level_fields(
        typed,
        data,
        members,
        &supplied,
        |field| {
            let current_occurrence = occurrences
                .get(occurrence_index)
                .zip(path_layout.paths.get(occurrence_index));
            let Some((occurrence, path)) = current_occurrence.filter(|(occurrence, _)| {
                field_occurrence_matches(
                    occurrence.outer_field(),
                    occurrence.outer_member_identity(),
                    field.name.as_str(),
                    field.identity,
                )
            }) else {
                return Ok(None);
            };
            occurrence_index += 1;
            let retained_inner_bytes = occurrence.inner.bytes();
            let mut staged_inner = Vec::new();
            staged_inner
                .try_reserve_exact(retained_inner_bytes.len())
                .map_err(|_| {
                    MaterializationDiagnostic("ConstMaterializable plural recursive inner staging exceeds compiler resources".to_owned())
                })?;
            staged_inner.extend_from_slice(retained_inner_bytes);
            Ok(Some(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .inner
                    .outer_layout()
                    .size
                    .expect("validated recursive inner extent"),
                align: path.inner.outer_layout().align,
                repeated: None,
                bytes: staged_inner,
            }))
        },
        &mut prepared_arrays,
        &mut prepared_record_arrays,
        &nested_sums,
        byte_order,
        "plural recursive",
    )?;
    if occurrence_index != occurrences.len() {
        return Err(MaterializationDiagnostic("ConstMaterializable plural recursive staging did not consume the complete authored-order set".to_owned()));
    }

    let bytes = materialize_level_bytes(&path_layout.outer_layout, encoded_fields, byte_order)?;
    let mut nested_sum_arrays = Vec::new();
    nested_sum_arrays
        .try_reserve_exact(prepared_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural recursive sum-array custody exceeds compiler resources"
                    .to_owned(),
            )
        })?;
    for prepared in prepared_arrays {
        nested_sum_arrays.push(ValidatedConstRecordSumArrayFieldMaterialization {
            field: prepared.field,
            field_identity: prepared.field_identity,
            elements: prepared.elements,
        });
    }
    let mut nested_record_arrays = Vec::new();
    nested_record_arrays
        .try_reserve_exact(prepared_record_arrays.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural recursive record-array custody exceeds compiler resources"
                    .to_owned(),
            )
        })?;
    for prepared in prepared_record_arrays {
        nested_record_arrays.push(ValidatedConstRecordArrayFieldMaterialization {
            field: prepared.field,
            field_identity: prepared.field_identity,
            elements: prepared.elements,
        });
    }
    Ok(DerivedRecursiveNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        nested_sums,
        nested_sum_arrays,
        nested_record_arrays,
        bytes,
    })
}

pub(super) fn derive_nested_record_sum_bytes(
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

pub(super) fn derive_nested_record_sums_bytes(
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

pub(super) fn derive_nested_record_sums_bytes_with_reachability(
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
            DataShapeKind::Enum | DataShapeKind::Mixed => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable nested-record path does not admit direct outer case-bearing field `{}`",
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
    materialize_aggregate_layout_into(
        path_layout.outer_layout(),
        &schemas,
        &values,
        byte_order,
        &mut bytes,
    )?;
    Ok(DerivedNestedRecordSumsMaterialization {
        schema_report_fingerprint,
        inner_records,
        bytes,
    })
}
