//! Recursive nested-record byte derivation and staging.

use super::{
    AggregateFieldSchema, AggregateFieldValue, BuildTimeValue, ByteOrder,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalRecordSumChildHop,
    ConventionalRecordSumChildInterior, ConventionalRecordSumChildLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumArrayFieldLayoutReport,
    DataDefinition, DataMember, DataShapeKind, DerivedNestedRecordSumMaterialization,
    DerivedNestedRecordSumsMaterialization, DerivedRecordLevelChildrenMaterialization,
    EncodedOuterField, MaterializationDiagnostic, NestedPathsView, RepeatedFieldInfo,
    SumReachability, TypedTrees, ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordArrayElementSelection, ValidatedConstRecordArrayFieldMaterialization,
    ValidatedConstRecordSumArrayFieldMaterialization, ValidatedConstRecordSumChildMaterialization,
    ValidatedConstRecordSumFieldMaterialization,
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

/// One classified direct child of a record level under the general
/// recursive rule — the value-side counterpart of the report's child row,
/// in authored member order.
pub(super) enum ClassifiedRecordLevelChild<'a> {
    /// A direct conventional pure-sum or mixed common-field/case field.
    Sum(&'a typed_trees::data::DataField, &'a DataDefinition),
    /// A direct nonzero literal fixed array of conventional pure sums. The
    /// `Vec<usize>` is every consecutive literal element arity, outermost
    /// first — `[[S; 2]; 3]` carries `[3, 2]` and its packed row's element
    /// count is the product.
    SumArray(
        &'a typed_trees::data::DataField,
        &'a DataDefinition,
        Vec<usize>,
    ),
    /// A direct nonzero literal fixed array of records still reaching sums
    /// inside the element, under the same flattened-hop rule.
    RecordArray(
        &'a typed_trees::data::DataField,
        &'a DataDefinition,
        Vec<usize>,
    ),
    /// A record field whose exact type still reaches sums below this level.
    Record(&'a typed_trees::data::DataField, &'a DataDefinition),
}

impl<'a> ClassifiedRecordLevelChild<'a> {
    /// The declared field the classified child spells.
    fn field(&self) -> &'a typed_trees::data::DataField {
        match self {
            Self::Sum(field, _)
            | Self::SumArray(field, ..)
            | Self::RecordArray(field, ..)
            | Self::Record(field, _) => field,
        }
    }

    /// The child kind's name in diagnostics.
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Sum(..) => "direct sum",
            Self::SumArray(..) => "sum array",
            Self::RecordArray(..) => "record array",
            Self::Record(..) => "record path",
        }
    }
}

/// Classify every runtime-relevant field of one record level exactly as the
/// projection does: direct pure sums, direct literal fixed arrays of pure
/// sums, direct literal fixed arrays of records still reaching sums inside
/// the element, and record fields still reaching sums below — one
/// authored-order channel whose rows keep their own kind. Consecutive
/// literal element hops flatten into the packed row's element count — a
/// `[[S; 2]; 3]` field is six packed sums — while mixed elements,
/// non-literal lengths, and zero-length hops remain fenced, and every
/// declared member must be supplied by the value.
pub(super) fn classify_record_level_children<'a>(
    typed: &'a TypedTrees,
    data: &'a DataDefinition,
    supplied: &std::collections::BTreeMap<&str, &BuildTimeValue>,
    reachability: &mut SumReachability<'_>,
) -> Result<Vec<ClassifiedRecordLevelChild<'a>>, MaterializationDiagnostic> {
    let members = typed.data_members(data);
    let mut children = Vec::new();
    children.try_reserve_exact(members.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable record-level child set exceeds compiler resources".into(),
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
                        children.push(ClassifiedRecordLevelChild::SumArray(field, named, hops));
                    }
                    // A record element crosses its own record boundary
                    // inside each index — the same literal element hop the
                    // projection admits, carried by the recursive report the
                    // row retains once for every element.
                    DataShapeKind::Record => {
                        children.push(ClassifiedRecordLevelChild::RecordArray(field, named, hops));
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
                    // coexists with the level's deeper record paths on the
                    // same authored-order child channel.
                    DataShapeKind::Enum | DataShapeKind::Mixed => {
                        children.push(ClassifiedRecordLevelChild::Sum(field, named));
                    }
                    DataShapeKind::Record => {
                        validate_outer_record_owner(typed, named)?;
                        children.push(ClassifiedRecordLevelChild::Record(field, named));
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

/// Validate one `[R; N]` child row — a literal index hop beside the shared
/// element record's recursive report — and every literal element's
/// recursive record/sum value, staging the field's complete repeated bytes
/// once. Each element keeps its literal index beside its own recursive
/// materialization coordinate; the complete element report lives once in
/// the row's interior.
pub(super) fn prepare_record_array_field(
    typed: &TypedTrees,
    array_field: &typed_trees::data::DataField,
    element_data: &DataDefinition,
    element_hops: &[usize],
    row: &ConventionalRecordSumChildLayoutReport,
    array_value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<PreparedRecordArrayField, MaterializationDiagnostic> {
    let (supplied_count, supplied_stride, inner) = match (&row.hop, &row.interior) {
        (
            ConventionalRecordSumChildHop::Index {
                element_count,
                element_stride,
            },
            ConventionalRecordSumChildInterior::Record(inner),
        ) => (*element_count, *element_stride, inner),
        _ => {
            unreachable!("callers dispatch record-array child rows by hop and interior")
        }
    };
    if !field_occurrence_matches(
        &row.field,
        row.member_identity,
        array_field.name.as_str(),
        array_field.identity,
    ) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable recursive level child row for `{}` is missing, duplicated, or out of authored field order",
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
    let element_size = inner.outer_layout.size.ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "ConstMaterializable recursive level record-array child row for `{}` requires one exact element extent",
            array_field.name
        ))
    })?;
    if supplied_count != element_count_u64 || supplied_stride != element_size {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable recursive level record-array count/stride drifted for `{}`",
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
    let total_size = supplied_stride.checked_mul(supplied_count).ok_or_else(|| {
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
        let inner_custody = recursive::validate_with_reachability(
            typed,
            element_data.name.as_str(),
            inner,
            element_value,
            byte_order,
            reachability,
        )?;
        if u64::try_from(inner_custody.bytes().len()).ok() != Some(element_size) {
            return Err(MaterializationDiagnostic(format!(
                "value.{} element {index} encoded to {} bytes, expected {element_size}",
                array_field.name,
                inner_custody.bytes().len()
            )));
        }
        let selection = ValidatedConstRecordArrayElementSelection {
            literal_index: u64::try_from(index).map_err(|_| {
                MaterializationDiagnostic(
                    "ConstMaterializable record-array index exceeds canonical width".into(),
                )
            })?,
            non_authoritative_materialization_report_fingerprint: inner_custody
                .non_authoritative_materialization_report_fingerprint(),
            value: element_value.clone(),
            bytes: inner_custody.bytes().to_vec(),
        };
        array_bytes.extend_from_slice(inner_custody.bytes());
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
        align: inner.outer_layout.align,
        repeated: RepeatedFieldInfo {
            element_size: supplied_stride,
            element_align: inner.outer_layout.align,
            element_count: supplied_count,
        },
    })
}

/// Stage one record level's runtime fields in authored member order,
/// drawing each classified child's already-encoded bytes off the single
/// `staged_children` cursor — the same authored-order channel the report
/// carries. Everything else encodes as an opaque aggregate value after
/// `validate_value`.
pub(super) fn stage_record_level_fields(
    typed: &TypedTrees,
    data: &DataDefinition,
    members: &[DataMember],
    supplied: &std::collections::BTreeMap<&str, &BuildTimeValue>,
    staged_children: &mut [EncodedOuterField],
    byte_order: ByteOrder,
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
    let mut staged_index = 0usize;
    for member in members {
        let DataMember::Field(field) = member else {
            unreachable!("outer record shape was validated above")
        };
        let field_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        if staged_children.get(staged_index).is_some_and(|staged| {
            field_occurrence_matches(
                field.name.as_str(),
                field.identity,
                &staged.name,
                staged.identity,
            )
        }) {
            let staged = &mut staged_children[staged_index];
            encoded_fields.push(EncodedOuterField {
                name: std::mem::take(&mut staged.name),
                identity: staged.identity,
                size: staged.size,
                align: staged.align,
                repeated: staged.repeated,
                bytes: std::mem::take(&mut staged.bytes),
            });
            staged_index += 1;
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
    if staged_index != staged_children.len() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable recursive level staging did not consume the complete authored-order child set"
                .to_owned(),
        ));
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

/// Derive one recursive record level's complete child custody from the
/// supplied report's single `children` channel: every supplied row is
/// validated against the level's classified child at the same authored
/// position and staged with its own encoded bytes, whether it is a direct
/// sum, a sum array, a record array, or a deeper record path.
pub(super) fn derive_record_level_children_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalRecursiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedRecordLevelChildrenMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable recursive level outer layout schema report fingerprint does not match `{schema_name}`"
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

    let classified = classify_record_level_children(typed, data, &supplied, reachability)?;
    if classified.is_empty() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable recursive level requires at least one direct runtime-relevant pure-sum, sum-array, record-array, or record field reaching sums"
                .to_owned(),
        ));
    }
    if path_layout.children.len() != classified.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable recursive level report contains {} child row(s), expected the complete authored-order set of {}",
            path_layout.children.len(),
            classified.len()
        )));
    }
    let leaf_occurrences = path_layout.leaf_occurrence_count().ok_or_else(|| {
        MaterializationDiagnostic(
            "ConstMaterializable recursive level leaf occurrence count overflows".to_owned(),
        )
    })?;
    if leaf_occurrences > SumReachability::MAX_EDGES {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable recursive level children exceed the global leaf occurrence bound"
                .to_owned(),
        ));
    }

    let mut custody = Vec::new();
    custody.try_reserve_exact(classified.len()).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable recursive level child custody exceeds compiler resources"
                .to_owned(),
        )
    })?;
    let mut staged_children = Vec::new();
    staged_children
        .try_reserve_exact(classified.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable recursive level child staging exceeds compiler resources"
                    .to_owned(),
            )
        })?;
    for (row, child) in path_layout.children.iter().zip(&classified) {
        let field = child.field();
        if !field_occurrence_matches(
            &row.field,
            row.member_identity,
            field.name.as_str(),
            field.identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable recursive level child row for `{}` is missing, duplicated, or out of authored field order",
                field.name
            )));
        }
        let row_value = supplied
            .get(field.name.as_str())
            .expect("complete outer value checked above");
        match (&row.hop, &row.interior, child) {
            (
                ConventionalRecordSumChildHop::Field,
                ConventionalRecordSumChildInterior::Sum(layout),
                ClassifiedRecordLevelChild::Sum(_, sum_data),
            ) => {
                let nested_sum = validate_const_materializable_conventional_sum(
                    typed,
                    sum_data.name.as_str(),
                    layout,
                    row_value,
                    byte_order,
                )?;
                staged_children.push(EncodedOuterField {
                    name: field.name.to_string(),
                    identity: field.identity,
                    size: layout.size,
                    align: layout.align,
                    repeated: None,
                    bytes: nested_sum.bytes().to_vec(),
                });
                custody.push(ValidatedConstRecordSumChildMaterialization::Sum(
                    ValidatedConstRecordSumFieldMaterialization {
                        field: field.name.to_string(),
                        field_identity: field.identity,
                        nested_sum,
                    },
                ));
            }
            (
                ConventionalRecordSumChildHop::Index {
                    element_count,
                    element_stride,
                },
                ConventionalRecordSumChildInterior::Sum(layout),
                ClassifiedRecordLevelChild::SumArray(_, sum_data, element_hops),
            ) => {
                let array_layout = ConventionalSumArrayFieldLayoutReport {
                    field: row.field.clone(),
                    member_identity: row.member_identity,
                    element_count: *element_count,
                    element_stride: *element_stride,
                    element_layout: layout.clone(),
                };
                let prepared = prepare_sum_array_field(
                    typed,
                    field,
                    sum_data,
                    element_hops,
                    &array_layout,
                    row_value,
                    byte_order,
                )?;
                staged_children.push(EncodedOuterField {
                    name: field.name.to_string(),
                    identity: field.identity,
                    size: prepared.size,
                    align: prepared.align,
                    repeated: Some(prepared.repeated),
                    bytes: prepared.bytes,
                });
                custody.push(ValidatedConstRecordSumChildMaterialization::SumArray(
                    ValidatedConstRecordSumArrayFieldMaterialization {
                        field: prepared.field,
                        field_identity: prepared.field_identity,
                        elements: prepared.elements,
                    },
                ));
            }
            (
                ConventionalRecordSumChildHop::Field,
                ConventionalRecordSumChildInterior::Record(inner),
                ClassifiedRecordLevelChild::Record(_, inner_data),
            ) => {
                let inner_custody = recursive::validate_with_reachability(
                    typed,
                    inner_data.name.as_str(),
                    inner,
                    row_value,
                    byte_order,
                    reachability,
                )?;
                let inner_size = inner.outer_layout.size.ok_or_else(|| {
                    MaterializationDiagnostic(format!(
                        "ConstMaterializable recursive level child row for `{}` requires one exact inner extent",
                        field.name
                    ))
                })?;
                if usize::try_from(inner_size).ok() != Some(inner_custody.bytes().len()) {
                    return Err(MaterializationDiagnostic(format!(
                        "ConstMaterializable recursive level inner bytes for `{}` do not cover the exact inner extent",
                        field.name
                    )));
                }
                staged_children.push(EncodedOuterField {
                    name: field.name.to_string(),
                    identity: field.identity,
                    size: inner_size,
                    align: inner.outer_layout.align,
                    repeated: None,
                    bytes: inner_custody.bytes().to_vec(),
                });
                custody.push(ValidatedConstRecordSumChildMaterialization::Record(
                    ValidatedConstRecursiveNestedSumOccurrenceMaterialization {
                        outer_field: field.name.to_string(),
                        outer_member_identity: field.identity,
                        inner: inner_custody,
                    },
                ));
            }
            (
                ConventionalRecordSumChildHop::Index { .. },
                ConventionalRecordSumChildInterior::Record(_),
                ClassifiedRecordLevelChild::RecordArray(_, element_data, element_hops),
            ) => {
                let prepared = prepare_record_array_field(
                    typed,
                    field,
                    element_data,
                    element_hops,
                    row,
                    row_value,
                    byte_order,
                    reachability,
                )?;
                staged_children.push(EncodedOuterField {
                    name: field.name.to_string(),
                    identity: field.identity,
                    size: prepared.size,
                    align: prepared.align,
                    repeated: Some(prepared.repeated),
                    bytes: prepared.bytes,
                });
                custody.push(ValidatedConstRecordSumChildMaterialization::RecordArray(
                    ValidatedConstRecordArrayFieldMaterialization {
                        field: prepared.field,
                        field_identity: prepared.field_identity,
                        elements: prepared.elements,
                    },
                ));
            }
            _ => {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable recursive level child row for `{}` drifts from its classified {} kind",
                    field.name,
                    child.kind_name()
                )));
            }
        }
    }

    let encoded_fields = stage_record_level_fields(
        typed,
        data,
        members,
        &supplied,
        &mut staged_children,
        byte_order,
    )?;
    let bytes = materialize_level_bytes(&path_layout.outer_layout, encoded_fields, byte_order)?;
    Ok(DerivedRecordLevelChildrenMaterialization {
        schema_report_fingerprint,
        children: custody,
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
