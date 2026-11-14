//! A policy sees the actual fields with their target-resolved extents. Schema
//! construction does not inspect source text or choose a replacement layout.

use layout_plans::{LayoutTargetFacts, StorageGeometry};
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{PlanLaidSchemaField, PlanLaidSchemaFieldKind, TypedTrees};

pub(super) fn record_fields(
    typed: &TypedTrees,
    data: &DataDefinition,
    target: &LayoutTargetFacts,
) -> Result<Vec<PlanLaidSchemaField>, String> {
    if data.quotient.is_some() || data.properties.carry.is_some() {
        return Err("layout reflection needs the selected quotient or carry representation".into());
    }
    let mut fields = Vec::new();
    for member in typed.data_members(data) {
        let DataMember::Field(field) = member else {
            return Err("a fixed record policy cannot omit the semantic type's sum cases".into());
        };
        if field.relevance.is_erased() {
            continue;
        }
        let geometry = field_geometry(typed, field.type_reference, target, 0)?;
        let reference = typed
            .type_reference_table
            .unconstrained_type_reference(field.type_reference);
        let kind = match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray { .. } => PlanLaidSchemaFieldKind::Repeated,
            _ if typed.primitive_type_reference(reference).is_some() => {
                PlanLaidSchemaFieldKind::Scalar
            }
            _ => PlanLaidSchemaFieldKind::Nested,
        };
        fields.push(PlanLaidSchemaField {
            schema_field: field.symbol,
            // Keys are local opaque coordinates, separate from stable member
            // identities. The checked result is normalized back to this roster.
            key: u64::try_from(fields.len())
                .ok()
                .and_then(|ordinal| ordinal.checked_add(1))
                .ok_or("layout field roster exceeds nonzero u64 keys")?,
            identity: field.identity,
            byte_size: geometry.byte_size,
            byte_alignment: geometry.byte_alignment,
            kind,
            layout_source: typed.field_layout_source(field.type_reference),
        });
    }
    if fields.iter().any(|field| field.identity.is_some()) {
        if fields.iter().any(|field| field.identity.is_none()) {
            return Err("layout schema mixes numbered and unnumbered physical fields".into());
        }
        fields.sort_unstable_by_key(|field| field.identity);
    }
    for pair in fields.windows(2) {
        if pair[0].identity.is_some() && pair[0].identity == pair[1].identity {
            return Err("layout schema repeats a stable member identity".into());
        }
    }
    if fields.iter().any(|field| {
        field
            .identity
            .is_some_and(|identity| data.retired_identities.contains(&identity))
    }) {
        return Err("layout schema reuses a retired member identity".into());
    }
    Ok(fields)
}

fn field_geometry(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    target: &LayoutTargetFacts,
    depth: usize,
) -> Result<StorageGeometry, String> {
    if depth >= layout_plans::CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
        return Err("layout schema exceeds the supported nested storage depth".into());
    }
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err("layout field lost its complete type".into());
    }
    if let Some(primitive) = typed.primitive_type_reference(reference) {
        let (byte_size, byte_alignment) = match primitive {
            PrimitiveType::Integer(carrier) => {
                let integer = semantic_vocabulary::ClosedIntegerType::from_fixed(carrier)
                    .ok_or("layout field integer has no supported native carrier")?;
                return Ok(layout_plans::field_integer_geometry(integer));
            }
            PrimitiveType::Bool => (1, 1),
            PrimitiveType::F32 => (4, 4),
            PrimitiveType::F64 => (8, 8),
            PrimitiveType::Addr => (target.pointer_size, target.pointer_alignment),
            PrimitiveType::Selector => {
                return Err("a selector has no default field representation".into());
            }
        };
        return Ok(StorageGeometry {
            byte_size,
            byte_alignment,
        });
    }
    let reference = typed
        .type_reference_table
        .unconstrained_type_reference(reference);
    if let Some(plan) = typed.plan_laid_layout_for_type(reference) {
        let layout = &plan
            .geometry_for_target(typed, Some(&target.profile))
            .ok_or("nested layout is unavailable for this exact target")?
            .validated_layout;
        return Ok(StorageGeometry {
            byte_size: layout
                .size
                .ok_or("dynamic nested layout has no fixed field extent")?,
            byte_alignment: layout.align,
        });
    }
    if typed.layout_storage_arguments(reference).is_some() {
        return Err("nested layout policy must finish before its containing schema".into());
    }
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let element = field_geometry(typed, *element_type, target, depth + 1)?;
            let count = u64::try_from(*length).map_err(|_| "layout array extent exceeds u64")?;
            Ok(StorageGeometry {
                byte_size: element
                    .byte_size
                    .checked_mul(count)
                    .ok_or("layout array byte extent overflows")?,
                byte_alignment: element.byte_alignment,
            })
        }
        TypeReferenceNode::Unit => Ok(StorageGeometry {
            byte_size: 0,
            byte_alignment: 1,
        }),
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            let data = typed
                .closed_data_definition(reference)
                .ok_or("layout field has no closed data definition")?;
            if data.quotient.is_some() || data.properties.carry.is_some() {
                return Err(
                    "nested layout needs its selected quotient or carry representation".into(),
                );
            }
            let mut common = Vec::new();
            let mut cases = Vec::new();
            for member in typed.data_members(data) {
                match member {
                    DataMember::Field(field) if !field.relevance.is_erased() => {
                        common.push(field_geometry(
                            typed,
                            field.type_reference,
                            target,
                            depth + 1,
                        )?);
                    }
                    DataMember::Variant(variant) => {
                        cases.push(
                            typed
                                .data_payload_fields(variant)
                                .iter()
                                .filter(|field| !field.relevance.is_erased())
                                .map(|field| {
                                    field_geometry(typed, field.type_reference, target, depth + 1)
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                    }
                    DataMember::Field(_) => {}
                }
            }
            if cases.is_empty() {
                layout_plans::conventional_record_geometry(common).map(|(_, geometry)| geometry)
            } else {
                layout_plans::conventional_sum_geometry(
                    StorageGeometry {
                        byte_size: target.case_tag_size,
                        byte_alignment: target.case_tag_alignment,
                    },
                    common,
                    cases,
                )
                .map(|geometry| geometry.storage)
            }
            .map_err(|error| format!("nested default field geometry: {error:?}"))
        }
        // Reference and slice storage must come from the same representation
        // selection as native emission; target pointer width alone is not one.
        _ => Err("layout field needs an established fixed representation".into()),
    }
}
