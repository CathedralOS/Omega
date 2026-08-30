//! Projection of the authoritative conventional pure-sum runtime layout.
//!
//! This is a report of the fixed tag-prefixed overlay selected by this crate,
//! not a back door for programmable `Layout` policies to author case/tag
//! placement.

use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{DataSupplyMode, Multiplicity};
use psi_layout_plans::{
    ConventionalSumArrayFieldLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceNode};

use crate::{DataShape, ENUM_TAG_BYTES, LayoutPlan, TypeLayoutDescriptor};

/// Project the bounded nested-sum materialization set from the exact target
/// runtime layout: one closed `[copy]` record with one or more direct,
/// runtime-relevant conventional pure-sum fields.
///
/// The outer report contains only whole-field `At` placements. The nested
/// reports remain compiler-owned tag/payload overlays; this function does not
/// expose programmable tag or case placement. Every nested report is paired
/// with its outer field name and stable member identity in authored runtime
/// field order, so repeated uses of the same sum type remain distinguishable.
/// Arrays of sums, recursively nested sums, and mixed data shapes reject.
pub fn project_conventional_record_with_sum_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<(LayoutPlanReport, Vec<ConventionalSumFieldLayoutReport>), Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "nested-sum record")?;
    if definition.supply_mode != DataSupplyMode::CheckedShape
        || definition.properties.multiplicity != Multiplicity::Unrestricted
        || !definition.type_parameters.is_empty()
        || !definition.lifetime_parameters.is_empty()
        || definition.generic_instance.is_some()
        || definition.quotient.is_some()
        || DataDefinition::shape_kind_from_members(program.data_members(definition))
            != DataShapeKind::Record
    {
        return Err(Diagnostic::error(format!(
            "nested-sum materialization owner `{}` must be one closed non-generic `[copy]` record",
            definition.name
        )));
    }

    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for nested-sum owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for nested-sum owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut nested_sums = Vec::new();
    let mut entries = Vec::with_capacity(declared_fields.len());
    let mut offsets = Vec::with_capacity(declared_fields.len());
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "nested-sum outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }
        if let Some(named) = exact_named_data(program, declared.type_reference)? {
            match DataDefinition::shape_kind_from_members(program.data_members(named)) {
                DataShapeKind::Enum => {
                    if laid.type_symbol != named.symbol {
                        return Err(Diagnostic::error(format!(
                            "target runtime layout field `{}` substitutes its nested sum type",
                            declared.name
                        )));
                    }
                    let nested_layout = project_conventional_sum_materialization_layout(
                        program,
                        plan,
                        named.symbol,
                    )?;
                    if laid.layout.size as u64 != nested_layout.size
                        || laid.layout.alignment as u64 != nested_layout.align
                    {
                        return Err(Diagnostic::error(format!(
                            "target runtime layout field `{}` does not retain the exact conventional sum extent/alignment",
                            declared.name
                        )));
                    }
                    nested_sums.push(ConventionalSumFieldLayoutReport {
                        field: declared.name.to_string(),
                        member_identity: declared.identity,
                        layout: nested_layout,
                    });
                }
                DataShapeKind::Mixed => {
                    return Err(Diagnostic::error(format!(
                        "nested-sum layout field `{}` uses a mixed common-field/case shape",
                        declared.name
                    )));
                }
                DataShapeKind::Empty | DataShapeKind::Record => {}
            }
        }
        let offset = laid.offset as u64;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    if nested_sums.is_empty() {
        return Err(Diagnostic::error(
            "nested-sum layout projection requires at least one direct runtime-relevant pure-sum field",
        ));
    }

    Ok((
        LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(data_layout.layout.size as u64),
            align: data_layout.layout.alignment as u64,
        },
        nested_sums,
    ))
}

/// Project the first compact fixed-array-of-conventional-sums rung.
///
/// The owner is the same exact closed `[copy]` record as the direct-field
/// projection, but it must contain exactly one runtime-relevant direct field
/// of type `[S; N]` where `N > 0` is literal and `S` is a conventional pure
/// sum. The complete sum layout is retained once with exact count/stride;
/// value-sensitive materialization retains the selected case separately for
/// each literal index.
pub fn project_conventional_record_with_sum_array_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<(LayoutPlanReport, ConventionalSumArrayFieldLayoutReport), Diagnostic> {
    let definition = unique_data_definition(program, data_symbol, "nested-sum array record")?;
    if definition.supply_mode != DataSupplyMode::CheckedShape
        || definition.properties.multiplicity != Multiplicity::Unrestricted
        || !definition.type_parameters.is_empty()
        || !definition.lifetime_parameters.is_empty()
        || definition.generic_instance.is_some()
        || definition.quotient.is_some()
        || DataDefinition::shape_kind_from_members(program.data_members(definition))
            != DataShapeKind::Record
    {
        return Err(Diagnostic::error(format!(
            "nested-sum array materialization owner `{}` must be one closed non-generic `[copy]` record",
            definition.name
        )));
    }

    let data_layout = unique_data_layout(plan, data_symbol, definition.name.as_str())?;
    let DataShape::Record {
        fields: laid_fields,
    } = data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "target runtime layout row for nested-sum array owner `{}` is not a record",
            definition.name
        )));
    };
    let declared_fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let laid_fields = plan.fields.span_or_empty(laid_fields);
    if declared_fields.len() != laid_fields.len() {
        return Err(Diagnostic::error(format!(
            "target runtime layout for nested-sum array owner `{}` has {} fields; checked schema has {} relevant fields",
            definition.name,
            laid_fields.len(),
            declared_fields.len()
        )));
    }

    let mut array_report = None;
    let mut entries = Vec::with_capacity(declared_fields.len());
    let mut offsets = Vec::with_capacity(declared_fields.len());
    for (declared, laid) in declared_fields.into_iter().zip(laid_fields) {
        if declared.symbol != laid.symbol || declared.name != laid.name {
            return Err(Diagnostic::error(format!(
                "target runtime layout field identity/order drifted at `{}`",
                declared.name
            )));
        }
        if plan.bit_field(declared.symbol).is_some()
            || plan.stored_integer(declared.symbol).is_some()
            || plan.repeated_field(declared.symbol).is_some()
        {
            return Err(Diagnostic::error(format!(
                "nested-sum array outer field `{}` uses target-dependent fragment, stored-integer, or repeated placement",
                declared.name
            )));
        }
        match program
            .type_reference_table
            .type_reference(declared.type_reference)
        {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                if let Some(named) = exact_named_data(program, *element_type)? {
                    match DataDefinition::shape_kind_from_members(program.data_members(named)) {
                        DataShapeKind::Enum => {
                            if *length == 0 {
                                return Err(Diagnostic::error(format!(
                                    "nested-sum array field `{}` must have nonzero literal length",
                                    declared.name
                                )));
                            }
                            if array_report.is_some() {
                                return Err(Diagnostic::error(
                                    "nested-sum array materialization permits exactly one direct fixed-array-of-sums field",
                                ));
                            }
                            let element_layout = project_conventional_sum_materialization_layout(
                                program,
                                plan,
                                named.symbol,
                            )?;
                            let TypeLayoutDescriptor::FixedArray {
                                element_type: laid_element,
                                length: laid_length,
                            } = &laid.type_descriptor
                            else {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` is not the exact declared fixed array",
                                    declared.name
                                )));
                            };
                            let TypeLayoutDescriptor::Named {
                                symbol: laid_symbol,
                                name: laid_name,
                            } = laid_element.as_ref()
                            else {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` substitutes its sum-array element type",
                                    declared.name
                                )));
                            };
                            if *laid_length != *length
                                || *laid_symbol != named.symbol
                                || laid.type_symbol != named.symbol
                                || laid_name.as_str() != named.name.as_str()
                            {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` substitutes its sum-array element/count",
                                    declared.name
                                )));
                            }
                            let stride = usize::try_from(element_layout.size).map_err(|_| {
                                Diagnostic::error(format!(
                                    "nested-sum array field `{}` element stride exceeds the compiler host",
                                    declared.name
                                ))
                            })?;
                            let element_count = u64::try_from(*length).map_err(|_| {
                                Diagnostic::error(format!(
                                    "nested-sum array field `{}` count exceeds canonical report width",
                                    declared.name
                                ))
                            })?;
                            let expected_size = stride.checked_mul(*length).ok_or_else(|| {
                                Diagnostic::error(format!(
                                    "nested-sum array field `{}` extent exceeds the compiler host",
                                    declared.name
                                ))
                            })?;
                            if laid.layout.size != expected_size
                                || laid.layout.alignment as u64 != element_layout.align
                            {
                                return Err(Diagnostic::error(format!(
                                    "target runtime layout field `{}` does not retain the exact repeated conventional sum extent/alignment",
                                    declared.name
                                )));
                            }
                            array_report = Some(ConventionalSumArrayFieldLayoutReport {
                                field: declared.name.to_string(),
                                member_identity: declared.identity,
                                element_count,
                                element_stride: element_layout.size,
                                element_layout,
                            });
                        }
                        DataShapeKind::Mixed => {
                            return Err(Diagnostic::error(format!(
                                "nested-sum array field `{}` uses mixed common-field/case elements",
                                declared.name
                            )));
                        }
                        DataShapeKind::Empty | DataShapeKind::Record => {}
                    }
                }
            }
            _ => {
                if let Some(named) = exact_named_data(program, declared.type_reference)?
                    && DataDefinition::shape_kind_from_members(program.data_members(named))
                        == DataShapeKind::Enum
                {
                    return Err(Diagnostic::error(
                        "nested-sum array materialization does not combine direct sum fields with the array occurrence",
                    ));
                }
            }
        }
        let offset = laid.offset as u64;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: declared.identity,
            placement: LayoutPlacementReport::At { offset },
        });
        offsets.push(offset);
    }
    let array_report = array_report.ok_or_else(|| {
        Diagnostic::error(
            "nested-sum array layout projection requires exactly one direct nonzero literal fixed-array-of-sums field",
        )
    })?;

    Ok((
        LayoutPlanReport {
            schema_report_fingerprint:
                psi_typed_trees::identity::normalized_schema_report_fingerprint(program, definition),
            entries,
            offsets: Some(offsets),
            size: Some(data_layout.layout.size as u64),
            align: data_layout.layout.alignment as u64,
        },
        array_report,
    ))
}

fn unique_data_definition<'a>(
    program: &'a CheckedTrees,
    data_symbol: SymbolHandle,
    role: &str,
) -> Result<&'a DataDefinition, Diagnostic> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == data_symbol);
    let definition = definitions.next().ok_or_else(|| {
        Diagnostic::error(format!("{role} names no exact checked data definition"))
    })?;
    if definitions.next().is_some() {
        return Err(Diagnostic::error(format!(
            "{role} data identity is ambiguous"
        )));
    }
    Ok(definition)
}

fn unique_data_layout<'a>(
    plan: &'a LayoutPlan,
    data_symbol: SymbolHandle,
    name: &str,
) -> Result<&'a crate::DataLayout, Diagnostic> {
    let mut layouts = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .filter(|layout| layout.symbol == data_symbol);
    let layout = layouts.next().ok_or_else(|| {
        Diagnostic::error(format!(
            "target runtime layout has no exact data row for `{name}`"
        ))
    })?;
    if layouts.next().is_some() {
        return Err(Diagnostic::error(format!(
            "target runtime layout has duplicate data rows for `{name}`"
        )));
    }
    Ok(layout)
}

fn exact_named_data<'a>(
    program: &'a CheckedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Result<Option<&'a DataDefinition>, Diagnostic> {
    if program.primitive_type_reference(type_reference).is_some() {
        return Ok(None);
    }
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return Ok(None);
    };
    if !symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "nested-sum field type `{name}` has no exact nominal identity"
        )));
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == *symbol);
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    if definitions.next().is_some() || definition.name.as_str() != name.as_str() {
        return Err(Diagnostic::error(format!(
            "nested-sum field type `{name}` has ambiguous or mismatched nominal identity"
        )));
    }
    Ok(Some(definition))
}

/// Project one exact closed pure sum from the already-built runtime layout.
/// Common-field/case mixed shapes reject and remain a separate materialization
/// rung.
pub fn project_conventional_sum_materialization_layout(
    program: &CheckedTrees,
    plan: &LayoutPlan,
    data_symbol: SymbolHandle,
) -> Result<ConventionalSumLayoutReport, Diagnostic> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == data_symbol);
    let definition = definitions.next().ok_or_else(|| {
        Diagnostic::error("conventional sum layout names no exact checked data definition")
    })?;
    if definitions.next().is_some() {
        return Err(Diagnostic::error(
            "conventional sum layout data identity is ambiguous",
        ));
    }
    let members = program.data_members(definition);
    if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Enum {
        return Err(Diagnostic::error(format!(
            "conventional sum materialization requires a pure sum; `{}` is empty, a record, or a mixed common-field/case shape",
            definition.name
        )));
    }

    let mut layouts = plan
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .filter(|layout| layout.symbol == data_symbol);
    let data_layout = layouts.next().ok_or_else(|| {
        Diagnostic::error(format!(
            "runtime layout has no exact data row for pure sum `{}`",
            definition.name
        ))
    })?;
    if layouts.next().is_some() {
        return Err(Diagnostic::error(format!(
            "runtime layout has duplicate data rows for pure sum `{}`",
            definition.name
        )));
    }
    let DataShape::Enum {
        common_fields,
        variants,
    } = &data_layout.shape
    else {
        return Err(Diagnostic::error(format!(
            "runtime layout row for pure sum `{}` is not case-bearing",
            definition.name
        )));
    };
    if !plan.fields.span_or_empty(*common_fields).is_empty() {
        return Err(Diagnostic::error(format!(
            "pure sum `{}` unexpectedly retains common runtime fields",
            definition.name
        )));
    }

    let declared_cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    let laid_cases = plan.variants.span_or_empty(*variants);
    if declared_cases.len() != laid_cases.len() {
        return Err(Diagnostic::error(format!(
            "pure sum `{}` runtime layout has {} cases; checked schema has {}",
            definition.name,
            laid_cases.len(),
            declared_cases.len()
        )));
    }

    let cases = declared_cases
        .into_iter()
        .zip(laid_cases)
        .enumerate()
        .map(|(ordinal, (declared, laid))| {
            if declared.symbol != laid.symbol || declared.name != laid.name {
                return Err(Diagnostic::error(format!(
                    "pure sum `{}` runtime case order or identity drifted at ordinal {ordinal}",
                    definition.name
                )));
            }
            let declared_payload = program
                .data_payload_fields(declared)
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            let laid_payload = plan.fields.span_or_empty(laid.fields);
            if declared_payload.len() != laid_payload.len() {
                return Err(Diagnostic::error(format!(
                    "pure sum `{}` case `{}` runtime payload has {} fields; checked schema has {} relevant fields",
                    definition.name,
                    declared.name,
                    laid_payload.len(),
                    declared_payload.len()
                )));
            }
            let payload_fields = declared_payload
                .into_iter()
                .zip(laid_payload)
                .map(|(declared_field, laid_field)| {
                    if declared_field.symbol != laid_field.symbol
                        || declared_field.name != laid_field.name
                    {
                        return Err(Diagnostic::error(format!(
                            "pure sum `{}` case `{}` runtime payload field identity or order drifted",
                            definition.name, declared.name
                        )));
                    }
                    Ok(ConventionalSumPayloadFieldLayoutReport {
                        field: declared_field.name.to_string(),
                        member_identity: declared_field.identity,
                        offset: laid_field.offset as u64,
                        size: laid_field.layout.size as u64,
                        align: laid_field.layout.alignment as u64,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(ConventionalSumCaseLayoutReport {
                case: declared.name.to_string(),
                member_identity: declared.identity,
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(format!(
                        "pure sum `{}` case ordinal exceeds u32",
                        definition.name
                    ))
                })?,
                payload_fields,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(ConventionalSumLayoutReport {
        schema_report_fingerprint: psi_typed_trees::identity::normalized_schema_report_fingerprint(
            program, definition,
        ),
        tag_offset: 0,
        tag_size: ENUM_TAG_BYTES as u64,
        tag_align: ENUM_TAG_BYTES as u64,
        cases,
        size: data_layout.layout.size as u64,
        align: data_layout.layout.alignment as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target::NativeTarget;
    use psi_build_time_evaluation::{
        BuildTimeValue, validate_const_materializable_conventional_sum,
        validate_const_materializable_record_with_conventional_sum,
        validate_const_materializable_record_with_conventional_sum_array,
    };
    use psi_checked_trees::{CheckFacts, CheckedTrees};
    use psi_layout_plans::{ByteOrder, normalized_conventional_sum_layout_report_fingerprint};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    fn checked(source: &str) -> CheckedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        CheckedTrees::with_roots(typed, CheckFacts::default())
    }

    #[test]
    fn projects_exact_authored_case_order_and_overlay_geometry() {
        let checked = checked(
            r#"
            data Choice [copy] {
                case Empty;
                case Number(value: u8, proof [erased]: u64);
                case Pair(left: u16, right: u32);
            }
            "#,
        );
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Choice")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host()).unwrap();
        let report =
            project_conventional_sum_materialization_layout(&checked, &plan, definition.symbol)
                .unwrap();

        assert_eq!(report.tag_offset, 0);
        assert_eq!(report.tag_size, 4);
        assert_eq!(report.tag_align, 4);
        assert_eq!(report.size, 12);
        assert_eq!(report.align, 4);
        assert_eq!(
            report
                .cases
                .iter()
                .map(|case| (case.case.as_str(), case.ordinal))
                .collect::<Vec<_>>(),
            [("Empty", 0), ("Number", 1), ("Pair", 2)]
        );
        assert!(report.cases[0].payload_fields.is_empty());
        assert_eq!(report.cases[1].payload_fields[0].offset, 4);
        assert_eq!(
            report.cases[2]
                .payload_fields
                .iter()
                .map(|field| (field.field.as_str(), field.offset, field.size))
                .collect::<Vec<_>>(),
            [("left", 4, 2), ("right", 8, 4)]
        );
        assert_ne!(
            normalized_conventional_sum_layout_report_fingerprint(&report),
            0
        );

        let value = BuildTimeValue::Case {
            variant: "Pair".into(),
            payload: vec![
                ("left".into(), BuildTimeValue::Int(0x1122)),
                ("right".into(), BuildTimeValue::Int(0x3344_5566)),
            ],
        };
        let materialized = validate_const_materializable_conventional_sum(
            &checked,
            "Choice",
            &report,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("authoritative runtime report should materialize its active case");
        assert_eq!(
            materialized.bytes(),
            &[2, 0, 0, 0, 0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33]
        );
    }

    #[test]
    fn mixed_common_field_shape_is_not_projected_as_a_pure_sum() {
        let checked = checked(
            r#"
            data Event [copy] {
                sequence: u8;
                case Ready(value: u16);
                case Waiting;
            }
            "#,
        );
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Event")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host()).unwrap();
        let error =
            project_conventional_sum_materialization_layout(&checked, &plan, definition.symbol)
                .unwrap_err();
        assert!(error.message.contains("pure sum"));
    }

    #[test]
    fn target_layout_projects_one_live_record_with_sum_materialization_pair() {
        let checked = checked(
            r#"
            data Choice [copy] {
                case Empty;
                case Pair(left: u16, right: u32);
            }
            data Envelope [copy] {
                prefix: u8;
                choice: Choice;
                suffix: u16;
            }
            "#,
        );
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Envelope")
            .unwrap();
        let plan = crate::build_layout_plan(&checked, NativeTarget::host()).unwrap();
        let (outer, nested_rows) = project_conventional_record_with_sum_materialization_layout(
            &checked,
            &plan,
            definition.symbol,
        )
        .expect("target runtime layout should project the exact paired evidence");
        assert_eq!(outer.offsets.as_deref(), Some(&[0, 4, 16][..]));
        assert_eq!(outer.size, Some(20));
        assert_eq!(outer.align, 4);
        assert_eq!(nested_rows.len(), 1);
        assert_eq!(nested_rows[0].field, "choice");
        assert_eq!(nested_rows[0].layout.size, 12);
        assert_eq!(nested_rows[0].layout.align, 4);

        let value = BuildTimeValue::Struct {
            type_name: "Envelope".into(),
            fields: vec![
                ("prefix".into(), BuildTimeValue::Int(7)),
                (
                    "choice".into(),
                    BuildTimeValue::Case {
                        variant: "Pair".into(),
                        payload: vec![
                            ("left".into(), BuildTimeValue::Int(0x1122)),
                            ("right".into(), BuildTimeValue::Int(0x3344_5566)),
                        ],
                    },
                ),
                ("suffix".into(), BuildTimeValue::Int(0x7788)),
            ],
        };
        let materialized = validate_const_materializable_record_with_conventional_sum(
            &checked,
            "Envelope",
            &outer,
            &nested_rows[0].layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("the target-produced pair should feed the nested-sum carrier");
        assert_eq!(
            materialized.bytes(),
            &[
                7, 0, 0, 0, 1, 0, 0, 0, 0x22, 0x11, 0, 0, 0x66, 0x55, 0x44, 0x33, 0x88, 0x77, 0, 0,
            ]
        );
    }

    #[test]
    fn target_layout_projects_every_direct_sum_occurrence_and_keeps_broader_shapes_fenced() {
        let checked = checked(
            r#"
            data Choice [copy] { case Empty; case Number(value: u8); }
            data Multiple [copy] { first: Choice; second: Choice; }
            data ErasedAlso [copy] { live: Choice; proof [erased]: Choice; }
            data ArrayOwner [copy] { choices: [Choice; 2]; }
            data ArrayWithNeighbor [copy] { bytes: [u8; 2]; choices: [Choice; 2]; suffix: u16; }
            data ZeroArrayOwner [copy] { choices: [Choice; 0]; }
            data TwoArrayOwner [copy] { first: [Choice; 1]; second: [Choice; 1]; }
            data Inner [copy] { choice: Choice; }
            data RecursiveOwner [copy] { inner: Inner; }
            data Mixed [copy] { common: u8; case Empty; }
            data MixedOwner [copy] { mixed: Mixed; }
            "#,
        );
        let plan = crate::build_layout_plan(&checked, NativeTarget::host()).unwrap();
        let multiple = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Multiple")
            .unwrap();
        let (outer, nested_rows) = project_conventional_record_with_sum_materialization_layout(
            &checked,
            &plan,
            multiple.symbol,
        )
        .expect("all direct runtime sum occurrences should project in authored order");
        assert_eq!(outer.offsets.as_deref(), Some(&[0, 8][..]));
        assert_eq!(
            nested_rows
                .iter()
                .map(|row| row.field.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(nested_rows[0].member_identity, None);
        assert_eq!(nested_rows[1].member_identity, None);
        assert_eq!(nested_rows[0].layout, nested_rows[1].layout);

        let erased_also = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "ErasedAlso")
            .unwrap();
        let (erased_outer, erased_rows) =
            project_conventional_record_with_sum_materialization_layout(
                &checked,
                &plan,
                erased_also.symbol,
            )
            .expect("erased sum fields are not runtime materialization occurrences");
        assert_eq!(erased_outer.offsets.as_deref(), Some(&[0][..]));
        assert_eq!(
            erased_rows
                .iter()
                .map(|row| row.field.as_str())
                .collect::<Vec<_>>(),
            ["live"]
        );

        let array_owner = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "ArrayOwner")
            .unwrap();
        let (array_outer, array_row) =
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &plan,
                array_owner.symbol,
            )
            .expect("one direct nonzero literal sum array should project compactly");
        assert_eq!(array_outer.offsets.as_deref(), Some(&[0][..]));
        assert_eq!(array_row.field, "choices");
        assert_eq!(array_row.member_identity, None);
        assert_eq!(array_row.element_count, 2);
        assert_eq!(array_row.element_stride, array_row.element_layout.size);
        assert_eq!(array_row.element_stride, 8);

        let array_data_layout =
            unique_data_layout(&plan, array_owner.symbol, "ArrayOwner").unwrap();
        let DataShape::Record {
            fields: array_fields,
        } = array_data_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let mut substituted_plan = plan.clone();
        substituted_plan.fields.span_mut_or_empty(array_fields)[0].type_symbol =
            SymbolHandle::invalid();
        assert!(
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &substituted_plan,
                array_owner.symbol,
            )
            .is_err(),
            "an inconsistent laid array element symbol must reject"
        );

        let neighbor_owner = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "ArrayWithNeighbor")
            .unwrap();
        let (neighbor_outer, neighbor_row) =
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &plan,
                neighbor_owner.symbol,
            )
            .expect("the compact target report should preserve ordinary sibling fields");
        let materialized = validate_const_materializable_record_with_conventional_sum_array(
            &checked,
            "ArrayWithNeighbor",
            &neighbor_outer,
            &neighbor_row,
            &BuildTimeValue::Struct {
                type_name: "ArrayWithNeighbor".into(),
                fields: vec![
                    (
                        "bytes".into(),
                        BuildTimeValue::Array(vec![
                            BuildTimeValue::Int(0xaa),
                            BuildTimeValue::Int(0xbb),
                        ]),
                    ),
                    (
                        "choices".into(),
                        BuildTimeValue::Array(vec![
                            BuildTimeValue::Case {
                                variant: "Empty".into(),
                                payload: Vec::new(),
                            },
                            BuildTimeValue::Case {
                                variant: "Number".into(),
                                payload: vec![("value".into(), BuildTimeValue::Int(0x5c))],
                            },
                        ]),
                    ),
                    ("suffix".into(), BuildTimeValue::Int(0x1122)),
                ],
            },
            ByteOrder::LittleEndian,
        )
        .expect("the target-produced compact report should rejoin indexed materialization");
        assert_eq!(neighbor_outer.offsets.as_deref(), Some(&[0, 4, 20][..]));
        assert_eq!(
            materialized.bytes(),
            &[
                0xaa, 0xbb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x5c, 0, 0, 0, 0x22, 0x11, 0,
                0,
            ]
        );
        let neighbor_data_layout =
            unique_data_layout(&plan, neighbor_owner.symbol, "ArrayWithNeighbor").unwrap();
        let DataShape::Record {
            fields: neighbor_fields,
        } = neighbor_data_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let neighbor_field_symbol = plan.fields.span_or_empty(neighbor_fields)[0].symbol;
        let mut repeated_neighbor_plan = plan.clone();
        repeated_neighbor_plan
            .repeated_fields
            .push(crate::RepeatedFieldLayout {
                field: neighbor_field_symbol,
                element_stride: 2,
            });
        assert!(
            project_conventional_record_with_sum_array_materialization_layout(
                &checked,
                &repeated_neighbor_plan,
                neighbor_owner.symbol,
            )
            .is_err(),
            "target-dependent repeated placement on a neighboring field must reject"
        );

        let multiple_layout = unique_data_layout(&plan, multiple.symbol, "Multiple").unwrap();
        let DataShape::Record {
            fields: multiple_fields,
        } = multiple_layout.shape
        else {
            unreachable!("fixture is a record")
        };
        let direct_field_symbol = plan.fields.span_or_empty(multiple_fields)[0].symbol;
        let mut repeated_direct_plan = plan.clone();
        repeated_direct_plan
            .repeated_fields
            .push(crate::RepeatedFieldLayout {
                field: direct_field_symbol,
                element_stride: 16,
            });
        assert!(
            project_conventional_record_with_sum_materialization_layout(
                &checked,
                &repeated_direct_plan,
                multiple.symbol,
            )
            .is_err(),
            "legacy direct projection must not flatten target-dependent outer placement"
        );

        for name in [
            "ZeroArrayOwner",
            "TwoArrayOwner",
            "RecursiveOwner",
            "MixedOwner",
        ] {
            let definition = checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap();
            assert!(
                project_conventional_record_with_sum_array_materialization_layout(
                    &checked,
                    &plan,
                    definition.symbol,
                )
                .is_err(),
                "{name} must remain outside the single direct nonzero sum-array rung"
            );
        }

        for name in ["ArrayOwner", "RecursiveOwner", "MixedOwner"] {
            let definition = checked
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
                .unwrap();
            assert!(
                project_conventional_record_with_sum_materialization_layout(
                    &checked,
                    &plan,
                    definition.symbol,
                )
                .is_err(),
                "{name} must remain outside the direct nested-sum rung"
            );
        }
    }
}
