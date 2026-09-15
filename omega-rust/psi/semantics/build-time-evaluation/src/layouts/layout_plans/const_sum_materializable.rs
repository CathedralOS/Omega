//! Value-sensitive materialization of one conventional closed pure sum.

use language_semantics::{DataSupplyMode, Multiplicity};
use layout_plans::{
    ByteOrder, ConventionalSumCaseLayoutReport, ConventionalSumLayoutReport,
    MaterializationDiagnostic, conventional_sum_layout_reports_match_for_replay,
    normalized_conventional_sum_layout_report_fingerprint,
};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind, DataVariant};

use super::const_materializable::{
    hash_byte, hash_bytes, hash_text, hash_u64, hash_value, unique_data_by_name, validate_value,
    value_kind,
};
use super::{
    BuildTimeValue, checked_align_up, encode_typed_owned_value,
    normalized_schema_report_fingerprint, reflected_nested_member_layout,
};

const CONVENTIONAL_TAG_SIZE: u64 = 4;
const CONVENTIONAL_TAG_ALIGN: u64 = 4;

/// Exact staged bytes for one active case under the compiler-owned
/// tag-prefixed overlay representation.
#[derive(Debug)]
pub struct ValidatedConstSumMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    layout: ConventionalSumLayoutReport,
    non_authoritative_layout_report_fingerprint: u64,
    selected_case_identity: Option<u64>,
    selected_case_ordinal: u32,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstSumMaterialization {
    pub(super) fn into_compact_selection(
        self,
    ) -> (u64, BuildTimeValue, u64, Option<u64>, u32, Vec<u8>, u64) {
        (
            self.non_authoritative_schema_report_fingerprint,
            self.value,
            self.non_authoritative_layout_report_fingerprint,
            self.selected_case_identity,
            self.selected_case_ordinal,
            self.bytes,
            self.non_authoritative_materialization_report_fingerprint,
        )
    }

    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Compact schema report coordinate. Exact replay resolves and walks the
    /// retained schema name in the caller's typed program.
    pub const fn non_authoritative_schema_report_fingerprint(&self) -> u64 {
        self.non_authoritative_schema_report_fingerprint
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn layout(&self) -> &ConventionalSumLayoutReport {
        &self.layout
    }

    /// Explicitly named accessor for the non-authoritative layout report
    /// coordinate.
    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }

    pub const fn selected_case_identity(&self) -> Option<u64> {
        self.selected_case_identity
    }

    pub const fn selected_case_ordinal(&self) -> u32 {
        self.selected_case_ordinal
    }

    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Explicitly named accessor for the non-authoritative materialization
    /// report coordinate.
    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &ConventionalSumLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable sum schema `{schema_name}` does not match retained schema `{}`",
                self.schema_name
            )));
        }
        if value != &self.value {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum value drifted from retained custody".into(),
            ));
        }
        if byte_order != self.byte_order {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum target byte order drifted from retained custody".into(),
            ));
        }
        let layout_report_fingerprint =
            normalized_conventional_sum_layout_report_fingerprint(layout);
        if layout_report_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !conventional_sum_layout_reports_match_for_replay(layout, &self.layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable conventional sum layout drifted from retained custody".into(),
            ));
        }
        let replayed = derive_sum_bytes(typed, schema_name, layout, value, byte_order)?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.selected_case_identity != self.selected_case_identity
            || replayed.selected_case_ordinal != self.selected_case_ordinal
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable selected sum case drifted from retained custody".into(),
            ));
        }
        if replayed.bytes != self.bytes {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum bytes drifted from exact zero-initialized replay".into(),
            ));
        }
        let materialization_report_fingerprint =
            non_authoritative_sum_materialization_report_fingerprint(
                schema_name,
                replayed.schema_report_fingerprint,
                layout_report_fingerprint,
                replayed.selected_case_identity,
                replayed.selected_case_ordinal,
                byte_order,
                value,
                &replayed.bytes,
            );
        if materialization_report_fingerprint
            != self.non_authoritative_materialization_report_fingerprint
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum report fingerprint drifted from exact replay".into(),
            ));
        }
        Ok(())
    }

    /// Replay before copying so rejection and a short destination are atomic.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.layout,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable sum copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Validate one closed non-generic `[copy]` pure-sum value against the exact
/// compiler-owned conventional runtime layout. This does not admit
/// programmable tag/case placement or mixed common-field shapes.
pub fn validate_const_materializable_conventional_sum(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &ConventionalSumLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstSumMaterialization, MaterializationDiagnostic> {
    let derived = derive_sum_bytes(typed, schema_name, layout, value, byte_order)?;
    let layout_report_fingerprint = normalized_conventional_sum_layout_report_fingerprint(layout);
    let materialization_report_fingerprint =
        non_authoritative_sum_materialization_report_fingerprint(
            schema_name,
            derived.schema_report_fingerprint,
            layout_report_fingerprint,
            derived.selected_case_identity,
            derived.selected_case_ordinal,
            byte_order,
            value,
            &derived.bytes,
        );
    Ok(ValidatedConstSumMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        layout: layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint,
        selected_case_identity: derived.selected_case_identity,
        selected_case_ordinal: derived.selected_case_ordinal,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_report_fingerprint,
    })
}

struct DerivedSumMaterialization {
    schema_report_fingerprint: u64,
    selected_case_identity: Option<u64>,
    selected_case_ordinal: u32,
    bytes: Vec<u8>,
}

fn derive_sum_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &ConventionalSumLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedSumMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_sum_owner(typed, data)?;
    let schema_report_fingerprint = normalized_schema_report_fingerprint(typed, data);
    validate_conventional_layout(typed, data, layout, schema_report_fingerprint)?;
    let (selected, selected_layout, payload) = selected_case(typed, data, layout, value)?;

    let mut active = vec![data.symbol];
    validate_selected_payload(typed, selected, payload, &mut active)?;
    let byte_len = usize::try_from(layout.size).map_err(|_| {
        MaterializationDiagnostic("ConstMaterializable sum extent exceeds compiler host".into())
    })?;
    let mut bytes = vec![0; byte_len];
    let tag_destination = bytes.get_mut(..4).ok_or_else(|| {
        MaterializationDiagnostic(
            "ConstMaterializable sum extent does not contain its conventional tag".into(),
        )
    })?;
    match byte_order {
        ByteOrder::LittleEndian => {
            tag_destination.copy_from_slice(&selected_layout.ordinal.to_le_bytes())
        }
        ByteOrder::BigEndian => {
            tag_destination.copy_from_slice(&selected_layout.ordinal.to_be_bytes())
        }
    }

    for (declared, field_layout) in typed
        .data_payload_fields(selected)
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .zip(&selected_layout.payload_fields)
    {
        let field_value = payload
            .iter()
            .find(|(name, _)| name == declared.name.as_str())
            .map(|(_, value)| value)
            .ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value::{} lost payload field `{}` after validation",
                    selected.name, declared.name
                ))
            })?;
        let encoded = encode_typed_owned_value(
            typed,
            declared.type_reference,
            field_value,
            byte_order,
            &mut vec![data.symbol],
        )?;
        if encoded.len() as u64 != field_layout.size {
            return Err(MaterializationDiagnostic(format!(
                "value::{} payload field `{}` encoded to {} bytes, expected {}",
                selected.name,
                declared.name,
                encoded.len(),
                field_layout.size
            )));
        }
        let start = usize::try_from(field_layout.offset).map_err(|_| {
            MaterializationDiagnostic("ConstMaterializable sum field offset exceeds host".into())
        })?;
        let end = start.checked_add(encoded.len()).ok_or_else(|| {
            MaterializationDiagnostic("ConstMaterializable sum field range overflows".into())
        })?;
        bytes
            .get_mut(start..end)
            .ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "value::{} payload field `{}` writes outside the conventional sum extent",
                    selected.name, declared.name
                ))
            })?
            .copy_from_slice(&encoded);
    }

    Ok(DerivedSumMaterialization {
        schema_report_fingerprint,
        selected_case_identity: selected.identity,
        selected_case_ordinal: selected_layout.ordinal,
        bytes,
    })
}

fn validate_sum_owner(
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
            "ConstMaterializable sum `{}` is generic, opaque, quotient, or lacks one exact closed checked-shape identity",
            data.name
        )));
    }
    if data.properties.multiplicity != Multiplicity::Unrestricted {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable sum `{}` is not `[copy]`",
            data.name
        )));
    }
    if DataDefinition::shape_kind_from_members(typed.data_members(data)) != DataShapeKind::Enum {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable conventional sum `{}` is empty, a record, or a mixed common-field/case shape",
            data.name
        )));
    }
    Ok(())
}

fn validate_conventional_layout(
    typed: &TypedTrees,
    data: &DataDefinition,
    layout: &ConventionalSumLayoutReport,
    schema_report_fingerprint: u64,
) -> Result<(), MaterializationDiagnostic> {
    if layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable sum layout schema report fingerprint does not match `{}`",
            data.name
        )));
    }
    if layout.tag_offset != 0
        || layout.tag_size != CONVENTIONAL_TAG_SIZE
        || layout.tag_align != CONVENTIONAL_TAG_ALIGN
    {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable sum layout drifted from the conventional 4-byte tag at offset zero"
                .into(),
        ));
    }
    let declared_cases = typed
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    if declared_cases.len() != layout.cases.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable sum layout has {} cases, expected {} for `{}`",
            layout.cases.len(),
            declared_cases.len(),
            data.name
        )));
    }

    let mut payload_align = 1u64;
    let mut case_shapes = Vec::with_capacity(declared_cases.len());
    for (ordinal, (declared, reported)) in declared_cases.iter().zip(&layout.cases).enumerate() {
        let expected_ordinal = u32::try_from(ordinal).map_err(|_| {
            MaterializationDiagnostic("ConstMaterializable sum has too many cases".into())
        })?;
        if reported.ordinal != expected_ordinal
            || reported.case != declared.name.as_str()
            || reported.member_identity != declared.identity
        {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable sum case identity/order drifted at ordinal {ordinal}"
            )));
        }
        let relevant = typed
            .data_payload_fields(declared)
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if relevant.len() != reported.payload_fields.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable sum case `{}` has {} reported payload fields, expected {}",
                declared.name,
                reported.payload_fields.len(),
                relevant.len()
            )));
        }
        let mut shapes = Vec::with_capacity(relevant.len());
        for (field, field_report) in relevant.into_iter().zip(&reported.payload_fields) {
            if field_report.field != field.name.as_str()
                || field_report.member_identity != field.identity
            {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable sum case `{}` payload identity/order drifted at `{}`",
                    declared.name, field.name
                )));
            }
            let (size, align) = reflected_nested_member_layout(
                typed,
                field.type_reference,
                &mut vec![data.symbol],
            )
            .ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "ConstMaterializable sum case `{}` payload field `{}` is outside the target-independent fixed aggregate subset",
                    declared.name, field.name
                ))
            })?;
            payload_align = payload_align.max(align);
            shapes.push((size, align, field_report));
        }
        case_shapes.push((declared, shapes));
    }

    let expected_align = CONVENTIONAL_TAG_ALIGN.max(payload_align);
    let payload_base = checked_align_up(CONVENTIONAL_TAG_SIZE, payload_align).ok_or_else(|| {
        MaterializationDiagnostic("ConstMaterializable sum payload base overflows".into())
    })?;
    let mut maximum_end = CONVENTIONAL_TAG_SIZE;
    for (case, fields) in case_shapes {
        let mut offset = payload_base;
        for (size, align, reported) in fields {
            offset = checked_align_up(offset, align).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "ConstMaterializable sum case `{}` payload alignment overflows",
                    case.name
                ))
            })?;
            if reported.offset != offset || reported.size != size || reported.align != align {
                return Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable sum case `{}` payload field `{}` geometry drifted",
                    case.name, reported.field
                )));
            }
            offset = offset.checked_add(size).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "ConstMaterializable sum case `{}` payload extent overflows",
                    case.name
                ))
            })?;
        }
        maximum_end = maximum_end.max(offset);
    }
    let expected_size = checked_align_up(maximum_end, expected_align).ok_or_else(|| {
        MaterializationDiagnostic("ConstMaterializable sum extent overflows".into())
    })?;
    if layout.align != expected_align || layout.size != expected_size {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable sum extent/alignment is {}/{}, expected {expected_size}/{expected_align}",
            layout.size, layout.align
        )));
    }
    Ok(())
}

fn selected_case<'a>(
    typed: &'a TypedTrees,
    data: &'a DataDefinition,
    layout: &'a ConventionalSumLayoutReport,
    value: &'a BuildTimeValue,
) -> Result<
    (
        &'a DataVariant,
        &'a ConventionalSumCaseLayoutReport,
        &'a [(String, BuildTimeValue)],
    ),
    MaterializationDiagnostic,
> {
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err(MaterializationDiagnostic(format!(
            "value expected a case of `{}`, found {}",
            data.name,
            value_kind(value)
        )));
    };
    let mut matches =
        typed
            .data_members(data)
            .iter()
            .enumerate()
            .filter_map(|(ordinal, member)| match member {
                DataMember::Variant(candidate) if candidate.name.as_str() == variant => {
                    Some((ordinal, candidate))
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            });
    let (ordinal, selected) = matches.next().ok_or_else(|| {
        MaterializationDiagnostic(format!(
            "value names unknown case `{variant}` of `{}`",
            data.name
        ))
    })?;
    if matches.next().is_some() {
        return Err(MaterializationDiagnostic(format!(
            "value names ambiguous case `{variant}` of `{}`",
            data.name
        )));
    }
    let selected_layout = layout.cases.get(ordinal).ok_or_else(|| {
        MaterializationDiagnostic("ConstMaterializable selected case has no layout row".into())
    })?;
    Ok((selected, selected_layout, payload))
}

fn validate_selected_payload(
    typed: &TypedTrees,
    selected: &DataVariant,
    payload: &[(String, BuildTimeValue)],
    active: &mut Vec<symbols::SymbolHandle>,
) -> Result<(), MaterializationDiagnostic> {
    let declared = typed.data_payload_fields(selected);
    if payload.len() != declared.len() {
        return Err(MaterializationDiagnostic(format!(
            "value::{} expected {} payload field(s), found {}",
            selected.name,
            declared.len(),
            payload.len()
        )));
    }
    for field in declared {
        let mut matches = payload
            .iter()
            .filter(|(name, _)| name == field.name.as_str());
        let (_, value) = matches.next().ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "value::{} is missing payload field `{}`",
                selected.name, field.name
            ))
        })?;
        if matches.next().is_some() {
            return Err(MaterializationDiagnostic(format!(
                "value::{} repeats payload field `{}`",
                selected.name, field.name
            )));
        }
        validate_value(
            typed,
            field.type_reference,
            value,
            &format!("value::{}.{}", selected.name, field.name),
            active,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn non_authoritative_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    layout_report_fingerprint: u64,
    selected_case_identity: Option<u64>,
    selected_case_ordinal: u32,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.const-materializable-sum.v1");
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, layout_report_fingerprint);
    match selected_case_identity {
        Some(identity) => {
            hash_byte(&mut hash, 1);
            hash_u64(&mut hash, identity);
        }
        None => hash_byte(&mut hash, 0),
    }
    hash_u64(&mut hash, u64::from(selected_case_ordinal));
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
mod tests;
