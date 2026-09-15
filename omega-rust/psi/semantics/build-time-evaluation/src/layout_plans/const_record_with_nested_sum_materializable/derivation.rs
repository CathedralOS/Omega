//! Recursive nested-record byte derivation and staging.

use super::{
    AggregateFieldSchema, AggregateFieldValue, BuildTimeValue, ByteOrder,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalRecordSumPathsLayoutReport,
    DataDefinition, DataMember, DataShapeKind, DerivedNestedRecordSumMaterialization,
    DerivedNestedRecordSumsMaterialization, DerivedRecursiveNestedSumsMaterialization,
    EncodedOuterField, MaterializationDiagnostic, NestedPathsView, SumReachability, TypedTrees,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization, encode_typed_owned_value,
    exact_named_data, exact_struct_fields, field_occurrence_matches,
    materialize_aggregate_layout_into, normalized_schema_report_fingerprint, record_sum_profile,
    recursive, reflected_field_layout, reject_sum_array_type, unique_data_by_name,
    validate_const_materializable_record_with_conventional_sums, validate_outer_layout,
    validate_outer_record_owner, validate_value, value_kind,
};
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

    let mut candidates = Vec::new();
    candidates.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural recursive occurrence set exceeds compiler resources"
                .to_owned(),
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
                    "ConstMaterializable plural recursive path does not admit direct outer sum field `{}`",
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
        return Err(MaterializationDiagnostic("ConstMaterializable plural recursive paths require a nonempty qualifying occurrence set".to_owned()));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural recursive report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
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

    let mut encoded_fields = Vec::new();
    encoded_fields
        .try_reserve_exact(members.len())
        .map_err(|_| {
            MaterializationDiagnostic("ConstMaterializable plural recursive outer field custody exceeds compiler resources".to_owned())
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural recursive active path exceeds compiler resources"
                .to_owned(),
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
            let retained_inner_bytes = occurrence.inner.bytes();
            let mut staged_inner = Vec::new();
            staged_inner
                .try_reserve_exact(retained_inner_bytes.len())
                .map_err(|_| {
                    MaterializationDiagnostic("ConstMaterializable plural recursive inner staging exceeds compiler resources".to_owned())
                })?;
            staged_inner.extend_from_slice(retained_inner_bytes);
            encoded_fields.push(EncodedOuterField {
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
        return Err(MaterializationDiagnostic("ConstMaterializable plural recursive staging did not consume the complete authored-order set".to_owned()));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated recursive outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural recursive outer extent exceeds compiler host".to_owned(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural recursive staged bytes exceed compiler resources"
                .to_owned(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural recursive schema staging exceeds compiler resources"
                    .to_owned(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural recursive value staging exceeds compiler resources"
                    .to_owned(),
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
    Ok(DerivedRecursiveNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
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
