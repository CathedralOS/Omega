//! Projection of the authoritative conventional pure-sum runtime layout.
//!
//! This is a report of the fixed tag-prefixed overlay selected by this crate,
//! not a back door for programmable `Layout` policies to author case/tag
//! placement.

use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_layout_plans::{
    ConventionalSumCaseLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};

use crate::{DataShape, ENUM_TAG_BYTES, LayoutPlan};

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
        schema_identity: psi_typed_trees::identity::normalized_schema_identity(program, definition),
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
    };
    use psi_checked_trees::{CheckFacts, CheckedTrees};
    use psi_layout_plans::{ByteOrder, normalized_conventional_sum_layout_fingerprint};
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
        assert_ne!(normalized_conventional_sum_layout_fingerprint(&report), 0);

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
}
