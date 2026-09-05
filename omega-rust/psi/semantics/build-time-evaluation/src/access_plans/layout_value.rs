use checked_interpreter::BuildTimeValue;
use layout_plans::{IntegerInterpretation, LayoutPlacementReport, LayoutPlanReport};

use crate::layout_plans::SchemaFieldInfo;

pub(super) fn build_layout_plan_value(
    layout: &LayoutPlanReport,
    schema_fields: &[SchemaFieldInfo],
) -> Result<BuildTimeValue, String> {
    let mut entries = Vec::with_capacity(64);
    for entry in &layout.entries {
        let schema_field = match entry.member_identity {
            Some(identity) => schema_fields
                .iter()
                .find(|field| field.identity == Some(identity)),
            None => schema_fields
                .iter()
                .find(|field| field.identity.is_none() && field.name == entry.field),
        }
        .ok_or_else(|| match entry.member_identity {
            Some(identity) => format!(
                "validated layout refers to numbered field `{}` with stable identity #{identity} outside the reflected schema",
                entry.field
            ),
            None => format!(
                "validated layout refers to positional field `{}` outside the reflected schema",
                entry.field
            ),
        })?;
        entries.push(BuildTimeValue::Struct {
            type_name: "FieldEntry".into(),
            fields: vec![
                ("key".into(), BuildTimeValue::Int(schema_field.key as i64)),
                (
                    "placement".into(),
                    match entry.placement {
                        LayoutPlacementReport::At { offset } => BuildTimeValue::Case {
                            variant: "At".into(),
                            payload: vec![("offset".into(), BuildTimeValue::Int(offset as i64))],
                        },
                        LayoutPlacementReport::IntegerAt {
                            offset,
                            stored_width,
                            interpretation,
                        } => BuildTimeValue::Case {
                            variant: "IntegerAt".into(),
                            payload: vec![
                                ("offset".into(), BuildTimeValue::Int(offset as i64)),
                                (
                                    "stored_width".into(),
                                    BuildTimeValue::Int(stored_width as i64),
                                ),
                                (
                                    "interpretation".into(),
                                    BuildTimeValue::Case {
                                        variant: match interpretation {
                                            IntegerInterpretation::Signed => "Signed".into(),
                                            IntegerInterpretation::Unsigned => "Unsigned".into(),
                                        },
                                        payload: Vec::new(),
                                    },
                                ),
                            ],
                        },
                        LayoutPlacementReport::Bits {
                            container,
                            container_width,
                            destination_lsb,
                            source_lsb,
                            width,
                        } => BuildTimeValue::Case {
                            variant: "Bits".into(),
                            payload: vec![
                                ("container".into(), BuildTimeValue::Int(container as i64)),
                                (
                                    "container_width".into(),
                                    BuildTimeValue::Int(container_width as i64),
                                ),
                                (
                                    "destination_lsb".into(),
                                    BuildTimeValue::Int(destination_lsb as i64),
                                ),
                                ("source_lsb".into(), BuildTimeValue::Int(source_lsb as i64)),
                                ("width".into(), BuildTimeValue::Int(width as i64)),
                            ],
                        },
                    },
                ),
            ],
        });
    }
    while entries.len() < 64 {
        entries.push(BuildTimeValue::Struct {
            type_name: "FieldEntry".into(),
            fields: vec![
                ("key".into(), BuildTimeValue::Int(0)),
                (
                    "placement".into(),
                    BuildTimeValue::Case {
                        variant: "At".into(),
                        payload: vec![("offset".into(), BuildTimeValue::Int(0))],
                    },
                ),
            ],
        });
    }
    Ok(BuildTimeValue::Struct {
        type_name: "Plan".into(),
        fields: vec![
            ("entries".into(), BuildTimeValue::Array(entries)),
            (
                "entry_count".into(),
                BuildTimeValue::Int(layout.entries.len() as i64),
            ),
            (
                "size_fixed".into(),
                BuildTimeValue::Int(layout.size.unwrap_or_default() as i64),
            ),
            (
                "size_is_dynamic".into(),
                BuildTimeValue::Bool(layout.size.is_none()),
            ),
            ("align".into(), BuildTimeValue::Int(layout.align as i64)),
        ],
    })
}
