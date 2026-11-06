//! Exact typed-schema identity and fixed runtime-geometry reflection.

use crate::build_time_evaluation::type_reference_table;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

mod schema_value;
pub(crate) use schema_value::{build_schema_value, build_selected_schema_value};

const SCHEMA_FIELD_CAPACITY: usize = 32;

pub(crate) fn local_schema_field_discriminator(schema: &str, field: &str) -> u64 {
    // Stable FNV-1a. This value is only a compiler-policy-local lookup key:
    // every reflected scope rejects collisions before exposing it, and exact
    // schema structure supplies durable identity. Zero remains the unused-tail
    // sentinel.
    let mut hash = semantic_vocabulary::Fnv1a::new();
    for byte in schema.bytes().chain(*b"::").chain(field.bytes()) {
        hash.byte(byte);
    }
    hash.finish_or(1)
}

#[derive(Debug, Clone)]
pub(crate) struct SchemaFieldInfo {
    pub(crate) schema_field: symbols::SymbolHandle,
    pub(crate) name: String,
    pub(crate) identity: Option<u64>,
    pub(crate) key: u64,
    pub(crate) size: u64,
    pub(crate) align: u64,
    pub(crate) source_bits: u64,
    /// Present only for scalar fields. Fixed arrays of primitive elements are
    /// reflected as one aggregate `At` placement and deliberately do not gain
    /// scalar integer/bit/access semantics.
    pub(crate) primitive: Option<PrimitiveType>,
    pub(crate) kind: &'static str,
    pub(crate) layout_source: typed_trees::PlanLaidFieldLayoutSource,
    pub(crate) declared_range: Option<(i64, i64)>,
    pub(crate) repeated: Option<RepeatedFieldInfo>,
    pub(crate) scalar_leaf_width_bits: Option<u16>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RepeatedFieldInfo {
    pub(crate) element_size: u64,
    pub(crate) element_count: u64,
}

/// Policy input order is independent of the semantic field/geometry roster.
/// Numbering is checked all-or-nothing during declaration formation.
pub(crate) fn canonical_schema_fields(fields: &[SchemaFieldInfo]) -> Vec<&SchemaFieldInfo> {
    let mut ordered = fields.iter().collect::<Vec<_>>();
    if ordered.iter().any(|field| field.identity.is_some()) {
        ordered.sort_unstable_by_key(|field| field.identity);
    }
    ordered
}

pub(crate) fn schema_fields(
    typed: &TypedTrees,
    schema_data: &str,
) -> Result<(Vec<SchemaFieldInfo>, u64), String> {
    let data = typed
        .data_definition_by_name_str(schema_data)
        .ok_or_else(|| format!("no data definition named `{schema_data}` exists"))?;
    schema_fields_for(typed, data)
}

pub(crate) fn schema_fields_for(
    typed: &TypedTrees,
    data: &DataDefinition,
) -> Result<(Vec<SchemaFieldInfo>, u64), String> {
    schema_fields_for_target(typed, data, None)
}

pub(crate) fn schema_fields_for_target(
    typed: &TypedTrees,
    data: &DataDefinition,
    target: Option<&layout_plans::LayoutTargetFacts>,
) -> Result<(Vec<SchemaFieldInfo>, u64), String> {
    let schema_data = data.name.as_str();
    if data.quotient.is_some() {
        return Err(format!(
            "schema reflection cannot observe quotient `{schema_data}`: retained representatives are opaque and require a named lifted operation"
        ));
    }

    let field_intervals = validation::data_where_field_intervals(typed, data);
    let mut fields = Vec::new();
    for member in typed.data_members(data) {
        let Some(field) = member.as_field() else {
            continue;
        };
        // Reflection describes physical placement demand. Erased bindings stay
        // in the semantic data definition and normalized schema identity, but
        // deliberately receive no field key and no plan entry.
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, source_bits, primitive, kind, declared_range, repeated) =
            reflected_field_layout(
                typed,
                field.type_reference,
                target,
                field_intervals.as_deref().and_then(|rows| rows.iter()
                    .find(|(symbol, _, _)| *symbol == field.symbol)
                    .map(|(_, low, high)| (*low, *high))),
            ).ok_or_else(|| {
                format!(
                    "schema data `{schema_data}` field `{}` is neither a supported primitive, a fixed array composed of supported primitives, nor a fixed record composed from those shapes",
                    field.name
                )
            })?;
        // A relevant field may itself be a checked record whose complete
        // runtime shape is erased. Keep the field in semantic/schema identity
        // and exact-value checking, but do not manufacture a zero-byte plan
        // entry for it.
        if size == 0 {
            continue;
        }
        let key = local_schema_field_discriminator(schema_data, field.name.as_str());
        if fields
            .iter()
            .any(|existing: &SchemaFieldInfo| existing.key == key)
        {
            return Err(format!(
                "schema data `{schema_data}` has a compiler-local field discriminator collision involving `{}`",
                field.name
            ));
        }
        fields.push(SchemaFieldInfo {
            schema_field: field.symbol,
            name: field.name.to_string(),
            identity: field.identity,
            key,
            size,
            align,
            source_bits,
            primitive,
            kind,
            layout_source: typed.field_layout_source(field.type_reference),
            declared_range,
            repeated,
            scalar_leaf_width_bits: scalar_leaf_width_bits(typed, field.type_reference),
        });
    }
    // An empty record has an empty physical schema, just like a record whose
    // fields all erase. Its type identity, qualifications and cleanup remain
    // semantic obligations; having no layout entries does not discard them.
    if fields.len() > SCHEMA_FIELD_CAPACITY {
        return Err(format!(
            "schema data `{schema_data}` has {} fields; the current layout slice supports at most {}",
            fields.len(),
            SCHEMA_FIELD_CAPACITY
        ));
    }
    if data.retired_identities.len() > SCHEMA_FIELD_CAPACITY {
        return Err(format!(
            "schema data `{schema_data}` has {} retired identities; reflected Schema supports at most {} per scope",
            data.retired_identities.len(),
            SCHEMA_FIELD_CAPACITY
        ));
    }
    Ok((fields, normalized_schema_report_fingerprint(typed, data)))
}

fn scalar_leaf_width_bits(typed: &TypedTrees, mut reference: TypeReferenceHandle) -> Option<u16> {
    loop {
        if let Some(primitive) = typed.primitive_type_reference(reference) {
            return u16::try_from(primitive_byte_size(primitive)?.checked_mul(8)?).ok();
        }
        reference = type_reference_table(typed).unconstrained_type_reference(reference);
        match typed.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                reference = *element_type
            }
            _ => return None,
        }
    }
}

pub fn normalized_schema_report_fingerprint(typed: &TypedTrees, data: &DataDefinition) -> u64 {
    typed_trees::identity::normalized_schema_report_fingerprint(typed, data)
}

/// Packing consumes the same declaration-owned field interval as Terminal
/// scalar restrictions. It refines geometry, without removing the declaration's
/// predicates or authorizing a write that cannot satisfy its physical codec.
fn declared_source_bits(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    primitive: PrimitiveType,
    byte_size: u64,
    stated: Option<(Option<i64>, Option<i64>)>,
) -> u64 {
    if primitive == PrimitiveType::Bool {
        return 1;
    }
    let width = byte_size * 8;
    if !primitive.is_integer() {
        return width;
    }
    let (mut minimum, mut maximum) = if primitive.is_signed_integer() {
        (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
    } else {
        (0, (1i128 << width) - 1)
    };
    if let Some(range) = validation::scalar_representation_range(typed, type_reference) {
        minimum = minimum.max(i128::from(range.minimum));
        maximum = maximum.min(i128::from(range.maximum));
    }
    if let Some((low, high)) = stated {
        if let Some(low) = low {
            minimum = minimum.max(i128::from(low));
        }
        if let Some(high) = high {
            maximum = maximum.min(i128::from(high));
        }
    }
    if minimum < 0 {
        return width;
    }
    let Ok(maximum) = u64::try_from(maximum) else {
        return width;
    };
    u64::from((u64::BITS - maximum.leading_zeros()).max(1))
}

pub(super) fn primitive_byte_size(primitive: PrimitiveType) -> Option<u64> {
    Some(match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 | PrimitiveType::Bool => 1,
        PrimitiveType::I16 | PrimitiveType::U16 => 2,
        PrimitiveType::I32 | PrimitiveType::U32 | PrimitiveType::F32 => 4,
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::F64 => 8,
        _ => return None,
    })
}

type ReflectedFieldLayout = (
    u64,
    u64,
    u64,
    Option<PrimitiveType>,
    &'static str,
    Option<(i64, i64)>,
    Option<RepeatedFieldInfo>,
);

pub(super) fn reflected_field_layout(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    target: Option<&layout_plans::LayoutTargetFacts>,
    stated: Option<(Option<i64>, Option<i64>)>,
) -> Option<ReflectedFieldLayout> {
    if let Some(primitive) = typed.primitive_type_reference(reference) {
        let size = primitive_byte_size(primitive)?;
        return Some((
            size,
            size,
            declared_source_bits(typed, reference, primitive, size, stated),
            Some(primitive),
            "Scalar",
            validation::scalar_representation_range(typed, reference)
                .map(|range| (range.minimum, range.maximum)),
            None,
        ));
    }
    let carrier = type_reference_table(typed).unconstrained_type_reference(reference);
    let (size, align) = reflected_member_layout(typed, carrier, target, &mut Vec::new())?;
    let repeated = match typed.type_reference_table.type_reference(carrier) {
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let (element_size, _) =
                reflected_member_layout(typed, *element_type, target, &mut Vec::new())?;
            Some(RepeatedFieldInfo {
                element_size,
                element_count: u64::try_from(*length).ok()?,
            })
        }
        _ => None,
    };
    Some((
        size,
        align,
        size.checked_mul(8)?,
        None,
        if repeated.is_some() {
            "Repeated"
        } else {
            "Nested"
        },
        None,
        repeated,
    ))
}

/// Walk the exact prepared field telescope and establish each by-value geometry.
/// Lifetimes and qualifications remain semantic evidence; references contribute
/// their descriptor geometry without demanding storage of the referent.
fn reflected_member_layout(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    target: Option<&layout_plans::LayoutTargetFacts>,
    visiting: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
) -> Option<(u64, u64)> {
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    if let Some(primitive) = typed.primitive_type_reference(reference) {
        let size = primitive_byte_size(primitive)?;
        return Some((size, size));
    }
    let carrier = type_reference_table(typed).unconstrained_type_reference(reference);
    match typed.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::Reference { referee, .. } => {
            let target = target?;
            let words = typed.reference_descriptor_word_count(*referee)?;
            Some((
                target.pointer_size.checked_mul(words)?,
                target.pointer_alignment,
            ))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let (size, align) = reflected_member_layout(typed, *element_type, target, visiting)?;
            Some((size.checked_mul(u64::try_from(*length).ok()?)?, align))
        }
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            if let Some(layout) = typed.plan_laid_layout_for_type(carrier) {
                let geometry = layout
                    .geometry_for_target(typed, target.map(|target| target.profile.as_str()))?;
                return Some((
                    geometry.validated_layout.size?,
                    geometry.validated_layout.align,
                ));
            }
            // A representation recipe without its admitted plan is pending,
            // never default-packed because its semantic fields are available.
            if typed.layout_storage_arguments(carrier).is_some() {
                return None;
            }
            let data = typed.data_field_definition(carrier)?;
            let identity = typed.normalized_type_identity(carrier);
            if data.quotient.is_some()
                || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                || visiting.contains(&identity)
            {
                return None;
            }
            visiting.push(identity);
            let result = (|| {
                let members = typed.data_members(data);
                if members.iter().any(|member| member.as_variant().is_some()) {
                    // A tag is a target representation, not a primitive-field
                    // assumption. Reuse native packing with explicitly supplied
                    // carrier geometry; never guess it from the executing host.
                    let target = target?;
                    target.validate().ok()?;
                    let field_geometry =
                        |field: &typed_trees::data::DataField, visiting: &mut Vec<_>| {
                            let (byte_size, byte_alignment) = reflected_member_layout(
                                typed,
                                field.type_reference,
                                Some(target),
                                visiting,
                            )?;
                            Some(layout_plans::StorageGeometry {
                                byte_size,
                                byte_alignment,
                            })
                        };
                    let mut common = Vec::new();
                    let mut cases = Vec::new();
                    for member in members {
                        match member {
                            typed_trees::data::DataMember::Field(field) => {
                                if !field.relevance.is_erased() {
                                    common.push(field_geometry(field, visiting)?);
                                }
                            }
                            typed_trees::data::DataMember::Variant(case) => {
                                let mut payload = Vec::new();
                                for field in typed.data_payload_fields(case) {
                                    if !field.relevance.is_erased() {
                                        payload.push(field_geometry(field, visiting)?);
                                    }
                                }
                                cases.push(payload);
                            }
                        }
                    }
                    let geometry = layout_plans::conventional_sum_geometry(
                        layout_plans::StorageGeometry {
                            byte_size: target.case_tag_size,
                            byte_alignment: target.case_tag_alignment,
                        },
                        common,
                        cases,
                    )
                    .ok()?
                    .storage;
                    return Some((geometry.byte_size, geometry.byte_alignment));
                }
                let mut offset = 0u64;
                let mut aggregate_align = 1u64;
                for member in typed.data_members(data) {
                    let field = member.as_field()?;
                    if field.relevance.is_erased() {
                        continue;
                    }
                    let (size, align) =
                        reflected_member_layout(typed, field.type_reference, target, visiting)?;
                    offset = checked_align_up(offset, align)?.checked_add(size)?;
                    aggregate_align = aggregate_align.max(align);
                }
                Some((checked_align_up(offset, aggregate_align)?, aggregate_align))
            })();
            visiting.pop();
            result
        }
        _ => None,
    }
}

pub(super) fn checked_align_up(value: u64, align: u64) -> Option<u64> {
    if align == 0 {
        return None;
    }
    value
        .checked_add(align - 1)
        .map(|value| value / align * align)
}

#[cfg(test)]
mod tests {
    use super::{reflected_member_layout, schema_fields_for};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::TypedTrees;
    use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
    use typed_trees::name::Identifier;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

    #[test]
    fn empty_record_has_an_empty_physical_schema() {
        let typed = TypedTrees::default();
        let record = DataDefinition::default();
        let (fields, fingerprint) = schema_fields_for(&typed, &record)
            .expect("an empty record needs no physical field entries");
        assert!(fields.is_empty());
        assert_eq!(
            fingerprint,
            typed_trees::identity::normalized_schema_report_fingerprint(&typed, &record)
        );
    }

    #[test]
    fn nested_sum_geometry_uses_selected_tag_and_preserves_array_stride() {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("geometry"));
        let sum = symbols
            .insert_children(root, [(SymbolKind::Data, SymbolNameRef::Static("Choice"))])
            .start();
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let byte = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: Identifier::from("u8"),
        });
        let word = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: Identifier::from("u64"),
        });
        let payload = typed.data_payload_fields.insert_many([DataField {
            name: Identifier::from("value"),
            type_reference: word,
            ..Default::default()
        }]);
        let members = typed.data_members.insert_many([
            DataMember::Field(DataField {
                name: Identifier::from("common"),
                type_reference: byte,
                ..Default::default()
            }),
            DataMember::Variant(DataVariant {
                name: Identifier::from("Empty"),
                ..Default::default()
            }),
            DataMember::Variant(DataVariant {
                name: Identifier::from("Held"),
                payload,
                ..Default::default()
            }),
        ]);
        typed.push_data_definition(DataDefinition {
            symbol: sum,
            name: Identifier::from("Choice"),
            members,
            ..Default::default()
        });
        let choice = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: sum,
            name: Identifier::from("Choice"),
        });
        let array = typed
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: choice,
                length: FixedArrayLength::Literal(2),
            });
        let mut target = layout_plans::LayoutTargetFacts {
            profile: "test".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        assert_eq!(
            reflected_member_layout(&typed, choice, None, &mut Vec::new()),
            None
        );
        assert_eq!(
            reflected_member_layout(&typed, choice, Some(&target), &mut Vec::new()),
            Some((16, 8))
        );
        assert_eq!(
            reflected_member_layout(&typed, array, Some(&target), &mut Vec::new()),
            Some((32, 8))
        );
        target.case_tag_size = 16;
        target.case_tag_alignment = 16;
        assert_eq!(
            reflected_member_layout(&typed, choice, Some(&target), &mut Vec::new()),
            Some((32, 16))
        );
        assert_eq!(
            reflected_member_layout(&typed, array, Some(&target), &mut Vec::new()),
            Some((64, 16))
        );
        target.case_tag_alignment = 3;
        assert_eq!(
            reflected_member_layout(&typed, choice, Some(&target), &mut Vec::new()),
            None
        );
    }
}
