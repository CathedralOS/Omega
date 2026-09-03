//! Fixed-depth nested-record byte derivation and staging.

use super::*;

pub(super) fn derive_depth_two_nested_sums_bytes_with_reachability(
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
            .checked_add(path.inner.paths.len())
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
                &path.inner,
                middle_value,
                byte_order,
                reachability,
            )?;
        let middle_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated middle extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_thirteen_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthThirteenNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-thirteen outer layout schema report fingerprint does not match `{schema_name}`"
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
            "ConstMaterializable plural depth-thirteen occurrence set exceeds compiler resources"
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
                    "ConstMaterializable plural depth-thirteen path does not admit direct outer sum field `{}`",
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
            "ConstMaterializable plural depth-thirteen paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-thirteen report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for twelfth_occurrence in &path.inner.paths {
            for eleventh_occurrence in &twelfth_occurrence.inner.paths {
                for tenth_occurrence in &eleventh_occurrence.inner.paths {
                    for ninth_occurrence in &tenth_occurrence.inner.paths {
                        for eighth_occurrence in &ninth_occurrence.inner.paths {
                            for seventh_occurrence in &eighth_occurrence.inner.paths {
                                for sixth_occurrence in &seventh_occurrence.inner.paths {
                                    for fifth_occurrence in &sixth_occurrence.inner.paths
                                    {
                                        for fourth_occurrence in
                                            &fifth_occurrence.inner.paths
                                        {
                                            for third_occurrence in
                                                &fourth_occurrence.inner.paths
                                            {
                                                for second_occurrence in
                                                    &third_occurrence.inner.paths
                                                {
                                                    total_leaf_occurrences = total_leaf_occurrences
                                                    .checked_add(
                                                        second_occurrence
                                                            .inner
                                                            .paths
                                                            .len(),
                                                    )
                                                    .ok_or_else(|| {
                                                        MaterializationDiagnostic(
                                                            "ConstMaterializable plural depth-thirteen leaf occurrence count overflows"
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
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen inner custody exceeds compiler resources"
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
                "ConstMaterializable plural depth-thirteen path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_twelve_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-thirteen path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-thirteen inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(
            ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization {
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
                "ConstMaterializable plural depth-thirteen outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-thirteen active path exceeds compiler resources"
                .into(),
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
                        "ConstMaterializable plural depth-thirteen inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-twelve extent"),
                align: path.inner.outer_layout.align,
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
            "ConstMaterializable plural depth-thirteen staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-thirteen outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-thirteen outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-thirteen staged bytes exceed compiler resources"
                .into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-thirteen value staging exceeds compiler resources"
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
    Ok(DerivedDepthThirteenNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

pub(super) fn derive_depth_fourteen_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthFourteenNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-fourteen outer layout schema report fingerprint does not match `{schema_name}`"
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
            "ConstMaterializable plural depth-fourteen occurrence set exceeds compiler resources"
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
                    "ConstMaterializable plural depth-fourteen path does not admit direct outer sum field `{}`",
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
            "ConstMaterializable plural depth-fourteen paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-fourteen report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for thirteenth_occurrence in &path.inner.paths {
            for twelfth_occurrence in &thirteenth_occurrence.inner.paths {
                for eleventh_occurrence in &twelfth_occurrence.inner.paths {
                    for tenth_occurrence in &eleventh_occurrence.inner.paths {
                        for ninth_occurrence in &tenth_occurrence.inner.paths {
                            for eighth_occurrence in &ninth_occurrence.inner.paths {
                                for seventh_occurrence in &eighth_occurrence.inner.paths
                                {
                                    for sixth_occurrence in
                                        &seventh_occurrence.inner.paths
                                    {
                                        for fifth_occurrence in
                                            &sixth_occurrence.inner.paths
                                        {
                                            for fourth_occurrence in
                                                &fifth_occurrence.inner.paths
                                            {
                                                for third_occurrence in
                                                    &fourth_occurrence.inner.paths
                                                {
                                                    for second_occurrence in
                                                        &third_occurrence.inner.paths
                                                    {
                                                        total_leaf_occurrences = total_leaf_occurrences
                                                    .checked_add(
                                                        second_occurrence
                                                            .inner
                                                            .paths
                                                            .len(),
                                                    )
                                                    .ok_or_else(|| {
                                                        MaterializationDiagnostic(
                                                            "ConstMaterializable plural depth-fourteen leaf occurrence count overflows"
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
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen inner custody exceeds compiler resources"
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
                "ConstMaterializable plural depth-fourteen path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_thirteen_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-fourteen path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-fourteen inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(
            ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization {
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
                "ConstMaterializable plural depth-fourteen outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-fourteen active path exceeds compiler resources"
                .into(),
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
                        "ConstMaterializable plural depth-fourteen inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-thirteen extent"),
                align: path.inner.outer_layout.align,
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
            "ConstMaterializable plural depth-fourteen staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-fourteen outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-fourteen outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-fourteen staged bytes exceed compiler resources"
                .into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-fourteen value staging exceeds compiler resources"
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
    Ok(DerivedDepthFourteenNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}
pub(super) fn derive_depth_fifteen_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthFifteenNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-fifteen outer layout schema report fingerprint does not match `{schema_name}`"
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
            "ConstMaterializable plural depth-fifteen occurrence set exceeds compiler resources"
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
                    "ConstMaterializable plural depth-fifteen path does not admit direct outer sum field `{}`",
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
            "ConstMaterializable plural depth-fifteen paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-fifteen report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for fourteenth_occurrence in &path.inner.paths {
            for thirteenth_occurrence in &fourteenth_occurrence.inner.paths {
                for twelfth_occurrence in &thirteenth_occurrence.inner.paths {
                    for eleventh_occurrence in &twelfth_occurrence.inner.paths {
                        for tenth_occurrence in &eleventh_occurrence.inner.paths {
                            for ninth_occurrence in &tenth_occurrence.inner.paths {
                                for eighth_occurrence in &ninth_occurrence.inner.paths {
                                    for seventh_occurrence in
                                        &eighth_occurrence.inner.paths
                                    {
                                        for sixth_occurrence in
                                            &seventh_occurrence.inner.paths
                                        {
                                            for fifth_occurrence in
                                                &sixth_occurrence.inner.paths
                                            {
                                                for fourth_occurrence in
                                                    &fifth_occurrence.inner.paths
                                                {
                                                    for third_occurrence in
                                                        &fourth_occurrence.inner.paths
                                                    {
                                                        for second_occurrence in
                                                            &third_occurrence.inner.paths
                                                        {
                                                            total_leaf_occurrences = total_leaf_occurrences
                                                    .checked_add(
                                                        second_occurrence
                                                            .inner
                                                            .paths
                                                            .len(),
                                                    )
                                                    .ok_or_else(|| {
                                                        MaterializationDiagnostic(
                                                            "ConstMaterializable plural depth-fifteen leaf occurrence count overflows"
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
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen inner custody exceeds compiler resources"
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
                "ConstMaterializable plural depth-fifteen path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_fourteen_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-fifteen path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-fifteen inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(
            ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization {
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
                "ConstMaterializable plural depth-fifteen outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-fifteen active path exceeds compiler resources"
                .into(),
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
                        "ConstMaterializable plural depth-fifteen inner staging exceeds compiler resources"
                            .into(),
                    )
                })?;
            inner_bytes.extend_from_slice(occurrence.inner.bytes());
            encoded_fields.push(EncodedOuterField {
                name: field.name.to_string(),
                identity: field.identity,
                size: path
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-fourteen extent"),
                align: path.inner.outer_layout.align,
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
            "ConstMaterializable plural depth-fifteen staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-fifteen outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-fifteen outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-fifteen staged bytes exceed compiler resources"
                .into(),
        )
    })?;
    bytes.resize(byte_len, 0);
    let mut schemas = Vec::new();
    let mut values = Vec::new();
    schemas
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-fifteen value staging exceeds compiler resources"
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
    Ok(DerivedDepthFifteenNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}

pub(super) fn derive_depth_twelve_nested_sums_bytes_with_reachability(
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
        for eleventh_occurrence in &path.inner.paths {
            for tenth_occurrence in &eleventh_occurrence.inner.paths {
                for ninth_occurrence in &tenth_occurrence.inner.paths {
                    for eighth_occurrence in &ninth_occurrence.inner.paths {
                        for seventh_occurrence in &eighth_occurrence.inner.paths {
                            for sixth_occurrence in &seventh_occurrence.inner.paths {
                                for fifth_occurrence in &sixth_occurrence.inner.paths {
                                    for fourth_occurrence in
                                        &fifth_occurrence.inner.paths
                                    {
                                        for third_occurrence in
                                            &fourth_occurrence.inner.paths
                                        {
                                            for second_occurrence in
                                                &third_occurrence.inner.paths
                                            {
                                                total_leaf_occurrences = total_leaf_occurrences
                                                    .checked_add(
                                                        second_occurrence
                                                            .inner
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-eleven extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_eleven_nested_sums_bytes_with_reachability(
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
        for tenth_occurrence in &path.inner.paths {
            for ninth_occurrence in &tenth_occurrence.inner.paths {
                for eighth_occurrence in &ninth_occurrence.inner.paths {
                    for seventh_occurrence in &eighth_occurrence.inner.paths {
                        for sixth_occurrence in &seventh_occurrence.inner.paths {
                            for fifth_occurrence in &sixth_occurrence.inner.paths {
                                for fourth_occurrence in &fifth_occurrence.inner.paths {
                                    for third_occurrence in
                                        &fourth_occurrence.inner.paths
                                    {
                                        for second_occurrence in
                                            &third_occurrence.inner.paths
                                        {
                                            total_leaf_occurrences = total_leaf_occurrences
                                        .checked_add(second_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-ten extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_ten_nested_sums_bytes_with_reachability(
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
        for ninth_occurrence in &path.inner.paths {
            for eighth_occurrence in &ninth_occurrence.inner.paths {
                for seventh_occurrence in &eighth_occurrence.inner.paths {
                    for sixth_occurrence in &seventh_occurrence.inner.paths {
                        for fifth_occurrence in &sixth_occurrence.inner.paths {
                            for fourth_occurrence in &fifth_occurrence.inner.paths {
                                for third_occurrence in &fourth_occurrence.inner.paths {
                                    for second_occurrence in &third_occurrence.inner.paths
                                    {
                                        total_leaf_occurrences = total_leaf_occurrences
                                        .checked_add(second_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-nine extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_nine_nested_sums_bytes_with_reachability(
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
        for eighth_occurrence in &path.inner.paths {
            for seventh_occurrence in &eighth_occurrence.inner.paths {
                for sixth_occurrence in &seventh_occurrence.inner.paths {
                    for fifth_occurrence in &sixth_occurrence.inner.paths {
                        for fourth_occurrence in &fifth_occurrence.inner.paths {
                            for third_occurrence in &fourth_occurrence.inner.paths {
                                for second_occurrence in &third_occurrence.inner.paths {
                                    total_leaf_occurrences = total_leaf_occurrences
                                        .checked_add(second_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-eight extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_eight_nested_sums_bytes_with_reachability(
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
        for seventh_occurrence in &path.inner.paths {
            for sixth_occurrence in &seventh_occurrence.inner.paths {
                for fifth_occurrence in &sixth_occurrence.inner.paths {
                    for fourth_occurrence in &fifth_occurrence.inner.paths {
                        for third_occurrence in &fourth_occurrence.inner.paths {
                            for second_occurrence in &third_occurrence.inner.paths {
                                total_leaf_occurrences = total_leaf_occurrences
                                    .checked_add(second_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-seven extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_seven_nested_sums_bytes_with_reachability(
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
        for sixth_occurrence in &path.inner.paths {
            for fifth_occurrence in &sixth_occurrence.inner.paths {
                for fourth_occurrence in &fifth_occurrence.inner.paths {
                    for third_occurrence in &fourth_occurrence.inner.paths {
                        for second_occurrence in &third_occurrence.inner.paths {
                            total_leaf_occurrences = total_leaf_occurrences
                                .checked_add(second_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-six extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_six_nested_sums_bytes_with_reachability(
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
        for fourth_occurrence in &path.inner.paths {
            for third_occurrence in &fourth_occurrence.inner.paths {
                for second_occurrence in &third_occurrence.inner.paths {
                    for first_occurrence in &second_occurrence.inner.paths {
                        total_leaf_occurrences = total_leaf_occurrences
                            .checked_add(first_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-five extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_five_nested_sums_bytes_with_reachability(
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
        for third_occurrence in &path.inner.paths {
            for second_occurrence in &third_occurrence.inner.paths {
                for first_occurrence in &second_occurrence.inner.paths {
                    total_leaf_occurrences = total_leaf_occurrences
                        .checked_add(first_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-four extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_four_nested_sums_bytes_with_reachability(
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
        for second_occurrence in &path.inner.paths {
            for first_occurrence in &second_occurrence.inner.paths {
                total_leaf_occurrences = total_leaf_occurrences
                    .checked_add(first_occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-three extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_three_nested_sums_bytes_with_reachability(
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
        for occurrence in &path.inner.paths {
            total_leaf_occurrences = total_leaf_occurrences
                .checked_add(occurrence.inner.paths.len())
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
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
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
                    .inner
                    .outer_layout
                    .size
                    .expect("validated plural depth-two extent"),
                align: path.inner.outer_layout.align,
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

pub(super) fn derive_depth_three_nested_sum_bytes(
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

pub(super) fn derive_depth_two_nested_sum_bytes(
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
