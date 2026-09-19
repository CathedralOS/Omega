//! Outer record owners, layouts and retained nested rows.

use crate::layouts::layout_plans::BuildTimeValue;
use crate::layouts::layout_plans::const_record_with_sum_materializable::ValidatedConstRecordSumFieldMaterialization;
use crate::layouts::layout_plans::const_record_with_sum_materializable::derived_bytes::EncodedOuterField;
use language_semantics::{DataSupplyMode, Multiplicity};
use layout_plans::{
    ByteOrder, ConventionalSumFieldLayoutReport, LayoutPlacementReport, LayoutPlanReport,
    MaterializationDiagnostic, conventional_sum_layout_reports_match_for_replay,
    normalized_conventional_sum_layout_report_fingerprint,
};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataShapeKind};
use typed_trees::types::TypeReferenceNode;

pub(crate) fn validate_outer_record_owner(
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

pub(crate) fn exact_named_data(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<Option<&DataDefinition>, MaterializationDiagnostic> {
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

pub(crate) fn validate_outer_layout(
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

pub(crate) fn validate_supplied_nested_rows_against_retained(
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

pub(crate) fn nested_sum_fields_match(
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

/// Collect one declared literal array's innermost element values in packed
/// order. `element_hops` carries each level's declared arity, outermost
/// first: a `[[T; 2]; 3]` field spells `[3, 2]` and yields six references in
/// the same order the compact row's flat element index uses — `field[i][j]`
/// is packed element `i * 2 + j`.
pub(crate) fn flatten_literal_array_elements<'v>(
    field_name: &str,
    element_hops: &[usize],
    array_value: &'v BuildTimeValue,
) -> Result<Vec<&'v BuildTimeValue>, MaterializationDiagnostic> {
    let mut level = vec![array_value];
    for hop in element_hops {
        let mut next = Vec::new();
        for value in level {
            let BuildTimeValue::Array(elements) = value else {
                return Err(MaterializationDiagnostic(format!(
                    "value.{field_name} is not a fixed array"
                )));
            };
            if elements.len() != *hop {
                return Err(MaterializationDiagnostic(format!(
                    "value.{field_name} has {} elements, expected {hop}",
                    elements.len()
                )));
            }
            next.extend(elements.iter());
        }
        level = next;
    }
    Ok(level)
}

pub(crate) fn field_occurrence_matches(
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
