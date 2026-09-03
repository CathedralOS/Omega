//! Exact plural depth-twenty-one constant-value custody and replay.

use super::depth_twenty::validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthTwentyOneRecordSumPathsLayoutReport;

fn depth_twenty_one_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-twenty-one-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-twenty-one path set.
///
/// The nested carrier retains its complete plural depth-twenty custody. This
/// type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
}

impl ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization {
        &self.inner
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-twenty carrier. This type deliberately
/// does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTwentyOneRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-twenty carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
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

    pub(super) fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !record_sum_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed = derive_depth_twenty_one_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one custody changed cardinality".into(),
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
                    "ConstMaterializable plural depth-twenty-one occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_with_reachability(
                typed,
                replayed.inner.schema_name(),
                &path.inner,
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
                    "ConstMaterializable plural depth-twenty-one inner custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one bytes drifted after exact replay"
                    .into(),
            ));
        }
        let fingerprint = depth_twenty_one_nested_sums_materialization_report_fingerprint(
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
                "ConstMaterializable plural depth-twenty-one fingerprint drifted after exact replay"
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
                "ConstMaterializable plural depth-twenty-one copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_twenty_one_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    let derived = derive_depth_twenty_one_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint =
        depth_twenty_one_nested_sums_materialization_report_fingerprint(
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
        ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization {
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

struct DerivedDepthTwentyOneNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

fn derive_depth_twenty_one_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTwentyOneNestedSumsMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    if path_layout.outer_layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-twenty-one outer layout schema report fingerprint does not match `{schema_name}`"
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
            "ConstMaterializable plural depth-twenty-one occurrence set exceeds compiler resources"
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
                    "ConstMaterializable plural depth-twenty-one path does not admit direct outer sum field `{}`",
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
            "ConstMaterializable plural depth-twenty-one paths require a nonempty qualifying occurrence set"
                .into(),
        ));
    }
    if path_layout.paths.len() != candidates.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural depth-twenty-one report contains {} occurrence(s), expected the complete authored-order set of {}",
            path_layout.paths.len(),
            candidates.len()
        )));
    }
    let mut total_leaf_occurrences = 0usize;
    for path in &path_layout.paths {
        for twentieth_occurrence in &path.inner.paths {
            for nineteenth_occurrence in &twentieth_occurrence.inner.paths {
                for eighteenth_occurrence in &nineteenth_occurrence.inner.paths {
                    for seventeenth_occurrence in &eighteenth_occurrence.inner.paths {
                        for sixteenth_occurrence in &seventeenth_occurrence.inner.paths {
                            for fifteenth_occurrence in &sixteenth_occurrence.inner.paths {
                                for fourteenth_occurrence in &fifteenth_occurrence.inner.paths {
                                    for thirteenth_occurrence in &fourteenth_occurrence.inner.paths
                                    {
                                        for twelfth_occurrence in &thirteenth_occurrence.inner.paths
                                        {
                                            for eleventh_occurrence in
                                                &twelfth_occurrence.inner.paths
                                            {
                                                for tenth_occurrence in
                                                    &eleventh_occurrence.inner.paths
                                                {
                                                    for ninth_occurrence in
                                                        &tenth_occurrence.inner.paths
                                                    {
                                                        for eighth_occurrence in
                                                            &ninth_occurrence.inner.paths
                                                        {
                                                            for seventh_occurrence in
                                                                &eighth_occurrence.inner.paths
                                                            {
                                                                for sixth_occurrence in
                                                                    &seventh_occurrence.inner.paths
                                                                {
                                                                    for fifth_occurrence in
                                                                        &sixth_occurrence
                                                                            .inner
                                                                            .paths
                                                                    {
                                                                        for fourth_occurrence in
                                                                            &fifth_occurrence
                                                                                .inner
                                                                                .paths
                                                                        {
                                                                            for third_occurrence in
                                                                                &fourth_occurrence
                                                                                    .inner
                                                                                    .paths
                                                                            {
                                                                                for second_occurrence in
                                                                                &third_occurrence
                                                                                    .inner
                                                                                    .paths
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
                                                            "ConstMaterializable plural depth-twenty-one leaf occurrence count overflows"
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
                            }
                        }
                    }
                }
            }
        }
        if total_leaf_occurrences > SumReachability::MAX_EDGES {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one paths exceed the global leaf occurrence bound"
                    .into(),
            ));
        }
    }

    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(candidates.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one inner custody exceeds compiler resources"
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
                "ConstMaterializable plural depth-twenty-one path for `{}` is missing, duplicated, or out of authored field order",
                inner_field.name
            )));
        }
        let inner_value = supplied
            .get(inner_field.name.as_str())
            .expect("complete outer value checked above");
        let inner =
            validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability(
                typed,
                inner_data.name.as_str(),
                &path.inner,
                inner_value,
                byte_order,
                reachability,
            )?;
        let inner_size = path.inner.outer_layout.size.ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-twenty-one path `{}` requires one exact inner extent",
                inner_field.name
            ))
        })?;
        if usize::try_from(inner_size).ok() != Some(inner.bytes().len()) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural depth-twenty-one inner bytes for `{}` do not cover the exact inner extent",
                inner_field.name
            )));
        }
        occurrences.push(
            ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization {
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
                "ConstMaterializable plural depth-twenty-one outer field custody exceeds compiler resources"
                    .into(),
            )
        })?;
    let mut active = Vec::new();
    active.try_reserve_exact(1).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-twenty-one active path exceeds compiler resources"
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
                        "ConstMaterializable plural depth-twenty-one inner staging exceeds compiler resources"
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
                    .expect("validated plural depth-twenty extent"),
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
            "ConstMaterializable plural depth-twenty-one staging did not consume the complete authored-order set"
                .into(),
        ));
    }

    validate_outer_layout(&path_layout.outer_layout, &encoded_fields)?;
    let byte_len = usize::try_from(
        path_layout
            .outer_layout
            .size
            .expect("validated plural depth-twenty-one outer extent"),
    )
    .map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-twenty-one outer extent exceeds compiler host".into(),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_len).map_err(|_| {
        MaterializationDiagnostic(
            "ConstMaterializable plural depth-twenty-one staged bytes exceed compiler resources"
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
                "ConstMaterializable plural depth-twenty-one schema staging exceeds compiler resources"
                    .into(),
            )
        })?;
    values
        .try_reserve_exact(encoded_fields.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable plural depth-twenty-one value staging exceeds compiler resources"
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
    Ok(DerivedDepthTwentyOneNestedSumsMaterialization {
        schema_report_fingerprint,
        occurrences,
        bytes,
    })
}
