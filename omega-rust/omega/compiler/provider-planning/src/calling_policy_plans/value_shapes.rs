//! Value shapes: deriving a boundary value shape from a type, an opaque
//! representation or a primitive, and classifying aggregates.

use crate::calling_policy_plans::boundary_signatures::{
    TraitTypeBinding, substituted_type_reference,
};
use crate::calling_policy_plans::build_time_decoding::{
    VALUE_FIELD_CAPACITY, VALUE_SHAPE_CAPACITY,
};
use crate::calling_policy_plans::opaque_representations::BoundaryOpaqueRepresentationUse;
use crate::calling_policy_plans::{
    BoundaryValueClass, BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
use build_time_evaluation::BuildTimeValue;
use calling_conventions::{CallingPolicy, SystemVEightbyteClass, ValueClass, ValueShape};
use representation_planning::{OpaqueRepresentationSelection, selection_for_opaque};
use target::NativeTarget;
use typed_trees::TypedTrees;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn value_shape_from_type(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<symbols::SymbolHandle>,
    shapes: &mut Vec<BoundaryValueShape>,
    fields: &mut Vec<BoundaryValueField>,
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    opaque_representations: &mut Vec<BoundaryOpaqueRepresentationUse>,
) -> Result<(ValueShape, u16), String> {
    type_reference = substituted_type_reference(typed, type_reference, bindings);
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        let abi = primitive_value_shape(primitive)?;
        let class = if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
            BoundaryValueClass::Float
        } else {
            BoundaryValueClass::Integer
        };
        let root = push_boundary_shape(
            shapes,
            BoundaryValueShape {
                class,
                byte_size: abi.byte_size,
                alignment: abi.alignment,
            },
        )?;
        return Ok((abi, root));
    }
    match typed.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => value_shape_from_type(
            typed,
            *base_type,
            bindings,
            visiting,
            shapes,
            fields,
            native_target,
            opaque_representation_selections,
            opaque_representations,
        ),
        TypeReferenceNode::Reference { referee, .. } => {
            let mut referee = substituted_type_reference(typed, *referee, bindings);
            while let TypeReferenceNode::Constrained { base_type, .. } =
                typed.type_reference_table.type_reference(referee)
            {
                referee = substituted_type_reference(typed, *base_type, bindings);
            }
            // An unsized referee needs a two-word descriptor: `{ptr, len}` for
            // slices and `string`, `{instance, table}` for `dyn Trait` — the
            // same pair ordinary dynamic-descriptor arguments expand to.
            let is_fat = match typed.type_reference_table.type_reference(referee) {
                TypeReferenceNode::Slice { .. } | TypeReferenceNode::DynamicTrait { .. } => true,
                TypeReferenceNode::Named { name, .. } => name.as_str() == "string",
                _ => false,
            };
            let pointer_size = u16::try_from(native_target.pointer_size)
                .map_err(|_| "target pointer size exceeds boundary shape range".to_owned())?;
            let pointer_alignment = u16::try_from(native_target.pointer_alignment)
                .map_err(|_| "target pointer alignment exceeds boundary shape range".to_owned())?;
            let byte_size = if is_fat {
                pointer_size
                    .checked_mul(2)
                    .ok_or_else(|| "fat reference boundary shape overflows".to_owned())?
            } else {
                pointer_size
            };
            let abi = ValueShape::integer(byte_size, pointer_alignment);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Reference,
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::Slice { .. } => {
            let byte_size = u16::try_from(native_target.pointer_size)
                .ok()
                .and_then(|size| size.checked_mul(2))
                .ok_or_else(|| {
                    "target slice-reference size exceeds boundary shape range".to_owned()
                })?;
            let alignment = u16::try_from(native_target.pointer_alignment)
                .map_err(|_| "target pointer alignment exceeds boundary shape range".to_owned())?;
            let abi = ValueShape::integer(byte_size, alignment);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Reference,
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let (element, element_root) = value_shape_from_type(
                typed,
                *element_type,
                bindings,
                visiting,
                shapes,
                fields,
                native_target,
                opaque_representation_selections,
                opaque_representations,
            )?;
            let size = usize::from(element.byte_size)
                .checked_mul(*length)
                .ok_or_else(|| "fixed-array boundary shape overflows".to_owned())?;
            let abi = ValueShape::integer(
                u16::try_from(size)
                    .map_err(|_| "fixed-array boundary shape exceeds 65535 bytes".to_owned())?,
                element.alignment,
            );
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::FixedArray {
                        element: element_root,
                        length: u16::try_from(*length).map_err(|_| {
                            "fixed-array boundary length exceeds 65535 elements".to_owned()
                        })?,
                    },
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::Named { symbol, name } => {
            let definition = typed
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol);
            if definition.is_some_and(|definition| {
                definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            }) {
                opaque_representation_value_shape(
                    typed,
                    *symbol,
                    name.as_str(),
                    bindings,
                    visiting,
                    shapes,
                    fields,
                    native_target,
                    opaque_representation_selections,
                    opaque_representations,
                )
            } else {
                plain_data_value_shape(
                    typed,
                    *symbol,
                    name.as_str(),
                    bindings,
                    visiting,
                    shapes,
                    fields,
                    native_target,
                    opaque_representation_selections,
                    opaque_representations,
                )
            }
        }
        TypeReferenceNode::DynamicTrait { .. } => {
            let byte_size = u16::try_from(native_target.pointer_size)
                .ok()
                .and_then(|size| size.checked_mul(2))
                .ok_or_else(|| {
                    "target dynamic-trait reference size exceeds boundary shape range".to_owned()
                })?;
            let alignment = u16::try_from(native_target.pointer_alignment)
                .map_err(|_| "target pointer alignment exceeds boundary shape range".to_owned())?;
            let abi = ValueShape::integer(byte_size, alignment);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Reference,
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::Generic { .. } => Err(format!(
            "generic value type `{}` is not concrete at the Calling<C> relationship",
            typed.display_type_reference(type_reference)
        )),
        TypeReferenceNode::FixedArray { .. } => Err(format!(
            "array type `{}` still has a non-literal length",
            typed.display_type_reference(type_reference)
        )),
        TypeReferenceNode::ConstExpression(_) => {
            Err("a proof-static index expression is not a boundary value shape".to_owned())
        }
        TypeReferenceNode::Unit => Err("unit is not a boundary value shape".to_owned()),
    }
}

pub(crate) fn opaque_representation_value_shape(
    typed: &TypedTrees,
    opaque: symbols::SymbolHandle,
    opaque_name: &str,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<symbols::SymbolHandle>,
    shapes: &mut Vec<BoundaryValueShape>,
    fields: &mut Vec<BoundaryValueField>,
    native_target: NativeTarget,
    selections: &[OpaqueRepresentationSelection],
    uses: &mut Vec<BoundaryOpaqueRepresentationUse>,
) -> Result<(ValueShape, u16), String> {
    if visiting.contains(&opaque) {
        return Err(format!(
            "opaque representation for `{opaque_name}` recursively contains its own semantic declaration"
        ));
    }
    let selection = selection_for_opaque(selections, opaque).ok_or_else(|| {
        format!(
            "boundary-opaque data `{opaque_name}` crosses this boundary by value, but the authoritative build selects no exact `OpaqueRepresentation<{opaque_name}>` conformance"
        )
    })?;
    let application = selection.application();
    let carrier = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == selection.carrier())
        .ok_or_else(|| {
            format!(
                "selected opaque representation carrier for `{opaque_name}` disappeared before shape closure"
            )
        })?;
    visiting.push(opaque);
    let shape = plain_data_value_shape(
        typed,
        carrier.symbol,
        carrier.name.as_str(),
        bindings,
        visiting,
        shapes,
        fields,
        native_target,
        selections,
        uses,
    );
    visiting.pop();
    let (abi, shape_root) = shape.map_err(|reason| {
        format!(
            "selected carrier `{}` cannot represent boundary-opaque `{opaque_name}` by value: {reason}",
            carrier.name,
        )
    })?;
    let use_identity = BoundaryOpaqueRepresentationUse {
        opaque,
        conformance: application.declaration,
        carrier: selection.carrier(),
        shape_root,
        application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment.as_bytes(),
        representation_schema_version: selection.schema_version(),
        origin: selection.origin(),
        lifecycle: selection.lifecycle(),
        copy_disposition: selection.copy_disposition(),
        selected_application_commitment: selection.selected_application_commitment(),
    };
    if !uses.contains(&use_identity) {
        uses.push(use_identity);
    }
    Ok((abi, shape_root))
}

/// Derive the exact address-free `Extent` graph for the currently admitted
/// UEFI/Microsoft program-storage source schema. A future SysV/AAPCS source
/// schema must retain and classify its own structural graph rather than
/// passing this fence.
pub fn selected_program_storage_source_extent_value_layout(
    typed: &TypedTrees,
    slot: target::ProgramEntrySlotDeclaration,
    type_reference: TypeReferenceHandle,
) -> Result<program_entry_plan::ProgramEntrySourceExtentValueLayout, String> {
    if slot.owner != target::TargetProfile::UefiX64
        || slot.schema != target::ProgramEntrySchema::ProgramStorageApplication
        || slot.visible_parameters != target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        || slot.semantic_calling_convention
            != Some(target::ProgramEntryCallingConvention::MicrosoftX64)
    {
        return Err(
            "selected-source Extent layout derivation is restricted to the exact UEFI/Microsoft program-storage schema"
                .into(),
        );
    }
    let mut shapes = Vec::new();
    let mut fields = Vec::new();
    let mut opaque_representations = Vec::new();
    let (shape, root) = value_shape_from_type(
        typed,
        type_reference,
        &[],
        &mut Vec::new(),
        &mut shapes,
        &mut fields,
        slot.owner.native_target(),
        &[],
        &mut opaque_representations,
    )?;
    let mut base_type = type_reference;
    while let TypeReferenceNode::Constrained {
        base_type: unconstrained,
        ..
    } = typed.type_reference_table.type_reference(base_type)
    {
        base_type = *unconstrained;
    }
    let TypeReferenceNode::Named {
        symbol: data_symbol,
        ..
    } = typed.type_reference_table.type_reference(base_type)
    else {
        return Err("selected program-storage root is not a named Extent record".into());
    };
    let definition = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *data_symbol)
        .ok_or_else(|| "selected program-storage Extent has no data definition".to_owned())?;
    let [
        typed_trees::data::DataMember::Field(base),
        typed_trees::data::DataMember::Field(length),
    ] = typed.data_members(definition)
    else {
        return Err("selected program-storage Extent must declare exactly two fields".into());
    };
    if base.relevance.is_erased()
        || length.relevance.is_erased()
        || base.name.as_str() != "base"
        || length.name.as_str() != "length"
        || typed.primitive_type_reference(base.type_reference) != Some(PrimitiveType::Addr)
        || typed.primitive_type_reference(length.type_reference) != Some(PrimitiveType::U64)
    {
        return Err(
            "selected program-storage Extent must declare exact `base: addr; length: u64` fields"
                .into(),
        );
    }
    let root = shapes
        .get(usize::from(root))
        .ok_or_else(|| "selected program-storage Extent lost its record shape root".to_owned())?;
    let BoundaryValueClass::Record {
        first_field,
        field_count: 2,
    } = root.class
    else {
        return Err(
            "selected program-storage Extent is not an exact two-field record graph".into(),
        );
    };
    let normalized_fields = fields
        .get(usize::from(first_field)..usize::from(first_field) + 2)
        .ok_or_else(|| "selected program-storage Extent field graph is out of bounds".to_owned())?;
    let [base_field, length_field] = normalized_fields else {
        unreachable!("exact Extent record graph retains two fields")
    };
    let scalar_shape = |field: &BoundaryValueField| -> Result<ValueShape, String> {
        let child = shapes.get(usize::from(field.shape)).ok_or_else(|| {
            "selected program-storage Extent child shape is out of bounds".to_owned()
        })?;
        if !matches!(child.class, BoundaryValueClass::Integer) {
            return Err("selected program-storage Extent field is not an integer scalar".into());
        }
        Ok(ValueShape::integer(child.byte_size, child.alignment))
    };
    program_entry_plan::ProgramEntrySourceExtentValueLayout::from_checked_record(
        *data_symbol,
        base.symbol,
        base_field.byte_offset,
        scalar_shape(base_field)?,
        length.symbol,
        length_field.byte_offset,
        scalar_shape(length_field)?,
        shape,
    )
}

pub(crate) fn push_boundary_shape(
    shapes: &mut Vec<BoundaryValueShape>,
    shape: BoundaryValueShape,
) -> Result<u16, String> {
    if shapes.len() >= VALUE_SHAPE_CAPACITY {
        return Err(format!(
            "boundary signature exceeds the normalized shape capacity of {VALUE_SHAPE_CAPACITY}"
        ));
    }
    let index = u16::try_from(shapes.len()).expect("boundary shape capacity fits u16");
    shapes.push(shape);
    Ok(index)
}

fn primitive_value_shape(primitive: PrimitiveType) -> Result<ValueShape, String> {
    let byte_size = primitive.scalar_byte_size().ok_or_else(|| {
        format!(
            "primitive `{}` has no concrete boundary size",
            primitive.name()
        )
    })?;
    let byte_size = u16::try_from(byte_size).expect("primitive size fits u16");
    Ok(match primitive {
        PrimitiveType::F32 | PrimitiveType::F64 => ValueShape::float(byte_size),
        _ => ValueShape::integer(byte_size, byte_size.clamp(1, 8)),
    })
}

pub(crate) fn plain_data_value_shape(
    typed: &TypedTrees,
    symbol: symbols::SymbolHandle,
    name: &str,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<symbols::SymbolHandle>,
    shapes: &mut Vec<BoundaryValueShape>,
    fields: &mut Vec<BoundaryValueField>,
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    opaque_representations: &mut Vec<BoundaryOpaqueRepresentationUse>,
) -> Result<(ValueShape, u16), String> {
    if visiting.contains(&symbol) {
        return Err(format!(
            "recursive by-value data `{name}` has no finite boundary shape"
        ));
    }
    let definition = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
        .ok_or_else(|| format!("named boundary type `{name}` has no data definition"))?;
    let planned_layout = typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_symbol == definition.symbol);
    let runtime_field_symbols = typed
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                Some(field.symbol)
            }
            typed_trees::data::DataMember::Field(_) | typed_trees::data::DataMember::Variant(_) => {
                None
            }
        })
        .collect::<Vec<_>>();
    if let Some(layout) = planned_layout
        && (layout.offsets.len() != runtime_field_symbols.len()
            || layout.field_symbols != runtime_field_symbols)
    {
        return Err(format!(
            "plan-laid data `{name}` has {} members but its layout publishes {} offsets",
            runtime_field_symbols.len(),
            layout.offsets.len()
        ));
    }
    visiting.push(symbol);
    let mut size = 0usize;
    let mut alignment = 1usize;
    let mut float_member_size = None;
    let mut float_members = 0usize;
    let mut record_fields = Vec::new();
    for member in typed.data_members(definition) {
        let typed_trees::data::DataMember::Field(field) = member else {
            visiting.pop();
            return Err(format!(
                "case data `{name}` is not yet classifiable as a boundary value; pass it by reference"
            ));
        };
        if field.relevance.is_erased() {
            continue;
        }
        let field_index = record_fields.len();
        let (shape, field_root) = if let Some(stored_integer) = planned_layout.and_then(|layout| {
            layout
                .integer_fields
                .iter()
                .find(|integer| integer.field_index == field_index)
        }) {
            let stored_byte_size = stored_integer.stored_width_bits / 8;
            if stored_integer.stored_width_bits == 0
                || stored_integer.stored_width_bits % 8 != 0
                || stored_byte_size > 8
            {
                visiting.pop();
                return Err(format!(
                    "plan-laid data `{name}` field `{}` has invalid stored-integer width {}",
                    field.name, stored_integer.stored_width_bits
                ));
            }
            let shape = ValueShape::integer(stored_byte_size, stored_byte_size);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                },
            )?;
            (shape, root)
        } else {
            value_shape_from_type(
                typed,
                field.type_reference,
                bindings,
                visiting,
                shapes,
                fields,
                native_target,
                opaque_representation_selections,
                opaque_representations,
            )?
        };
        let field_alignment = usize::from(shape.alignment);
        let field_offset = planned_layout.map_or_else(
            || align_up(size, field_alignment),
            |layout| layout.offsets[field_index],
        );
        size = size.max(
            field_offset
                .checked_add(usize::from(shape.byte_size))
                .ok_or_else(|| format!("data `{name}` boundary layout overflows"))?,
        );
        record_fields.push(BoundaryValueField {
            shape: field_root,
            byte_offset: u16::try_from(field_offset)
                .map_err(|_| format!("data `{name}` field offset exceeds 65535 bytes"))?,
        });
        alignment = alignment.max(field_alignment);
        match shape.class {
            ValueClass::Float
                if float_members != usize::MAX
                    && float_member_size.is_none_or(|prior| prior == shape.byte_size) =>
            {
                float_member_size = Some(shape.byte_size);
                float_members += 1;
            }
            _ => float_members = usize::MAX,
        }
    }
    visiting.pop();
    if let Some(layout) = planned_layout {
        size = layout.size;
        alignment = layout.align;
    } else {
        size = align_up(size, alignment);
    }
    if size == 0 {
        return Err(format!(
            "zero-sized data `{name}` cannot cross a boundary by value"
        ));
    }
    let size = u16::try_from(size)
        .map_err(|_| format!("data `{name}` boundary shape exceeds 65535 bytes"))?;
    let alignment = u16::try_from(alignment)
        .map_err(|_| format!("data `{name}` boundary alignment exceeds 65535 bytes"))?;
    let abi = if (1..=4).contains(&float_members)
        && float_member_size.is_some_and(|member_size| {
            usize::from(size) == usize::from(member_size) * float_members
                && alignment == member_size
                && record_fields.iter().enumerate().all(|(index, field)| {
                    usize::from(field.byte_offset) == index * usize::from(member_size)
                })
        }) {
        ValueShape::homogeneous_float_aggregate(
            float_member_size.expect("float member count has size"),
            u8::try_from(float_members).expect("HFA member count fits u8"),
        )
    } else {
        ValueShape::integer(size, alignment)
    };
    if fields.len().saturating_add(record_fields.len()) > VALUE_FIELD_CAPACITY {
        return Err(format!(
            "boundary signature exceeds the normalized field capacity of {VALUE_FIELD_CAPACITY}"
        ));
    }
    let first_field = u16::try_from(fields.len()).expect("boundary field capacity fits u16");
    let field_count = u16::try_from(record_fields.len())
        .map_err(|_| format!("data `{name}` has more than 65535 boundary fields"))?;
    fields.extend(record_fields);
    let root = push_boundary_shape(
        shapes,
        BoundaryValueShape {
            class: BoundaryValueClass::Record {
                first_field,
                field_count,
            },
            byte_size: size,
            alignment,
        },
    )?;
    Ok((abi, root))
}

fn align_up(value: usize, alignment: usize) -> usize {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|value| value / alignment * alignment)
        .unwrap_or(usize::MAX)
}

pub(crate) fn classify_boundary_aggregate(
    signature: &MaterializedBoundarySignature,
    root: u16,
    policy: CallingPolicy,
) -> Result<ValueClass, String> {
    if policy == CallingPolicy::MicrosoftX64 {
        return Ok(ValueClass::Integer);
    }
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    let mut leaves = Vec::new();
    collect_boundary_scalar_leaves(signature, root, 0, &mut leaves, 0)?;
    if let Some(members) = homogeneous_float_leaf_shape(shape, &leaves) {
        if policy == CallingPolicy::Aapcs64
            || (policy == CallingPolicy::SystemVAMD64 && members > 1)
        {
            return Ok(ValueClass::HomogeneousFloatAggregate { members });
        }
        if policy == CallingPolicy::SystemVAMD64 && members == 1 {
            return Ok(ValueClass::Float);
        }
    }
    if policy != CallingPolicy::SystemVAMD64 || shape.byte_size > 16 {
        return Ok(ValueClass::Integer);
    }
    let mut classes = [None, None];
    for &(offset, byte_size, is_float) in &leaves {
        let eightbyte = usize::from(offset) / 8;
        let last = usize::from(offset)
            .checked_add(usize::from(byte_size))
            .and_then(|end| end.checked_sub(1))
            .ok_or_else(|| "aggregate contains a zero-width scalar leaf".to_owned())?;
        if eightbyte > 1 || eightbyte != last / 8 {
            return Err(
                "aggregate has a scalar leaf crossing a SysV eightbyte boundary".to_owned(),
            );
        }
        classes[eightbyte] = Some(match classes[eightbyte] {
            Some(existing_is_sse) => existing_is_sse && is_float,
            None => is_float,
        });
    }
    let first = classes[0].ok_or_else(|| "aggregate has no scalar leaves".to_owned())?;
    let second = if shape.byte_size > 8 {
        classes[1].ok_or_else(|| "aggregate leaves do not cover its second eightbyte".to_owned())?
    } else {
        false
    };
    if !first && !second {
        return Ok(ValueClass::Integer);
    }
    if shape.byte_size <= 8 {
        return Err(
            "one-eightbyte non-homogeneous SSE aggregates are not representable in the closed ABI vocabulary"
                .to_owned(),
        );
    }
    Ok(ValueClass::SystemVAggregate {
        first: if first {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
        second: if second {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
    })
}

fn homogeneous_float_leaf_shape(
    aggregate: &BoundaryValueShape,
    leaves: &[(u16, u16, bool)],
) -> Option<u8> {
    let &(first_offset, member_size, true) = leaves.first()? else {
        return None;
    };
    let members = u8::try_from(leaves.len()).ok()?;
    (first_offset == 0
        && (1..=4).contains(&members)
        && matches!(member_size, 4 | 8)
        && leaves
            .iter()
            .enumerate()
            .all(|(index, &(offset, size, is_float))| {
                is_float
                    && size == member_size
                    && usize::from(offset) == index * usize::from(member_size)
            })
        && aggregate.byte_size == member_size * u16::from(members)
        && aggregate.alignment == member_size)
        .then_some(members)
}

fn collect_boundary_scalar_leaves(
    signature: &MaterializedBoundarySignature,
    root: u16,
    base_offset: u16,
    leaves: &mut Vec<(u16, u16, bool)>,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("boundary shape nesting exceeds 32 levels".to_owned());
    }
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    match shape.class {
        BoundaryValueClass::Integer | BoundaryValueClass::Reference => {
            leaves.push((base_offset, shape.byte_size, false));
        }
        BoundaryValueClass::Float => leaves.push((base_offset, shape.byte_size, true)),
        BoundaryValueClass::FixedArray { element, length } => {
            let element_shape = signature
                .shapes
                .get(usize::from(element))
                .ok_or_else(|| format!("array element shape {element} is outside the graph"))?;
            for index in 0..length {
                let offset = element_shape
                    .byte_size
                    .checked_mul(index)
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| "fixed-array leaf offset overflows u16".to_owned())?;
                collect_boundary_scalar_leaves(signature, element, offset, leaves, depth + 1)?;
            }
        }
        BoundaryValueClass::Record {
            first_field,
            field_count,
        } => {
            let start = usize::from(first_field);
            let end = start
                .checked_add(usize::from(field_count))
                .ok_or_else(|| "record field range overflows".to_owned())?;
            let fields = signature
                .fields
                .get(start..end)
                .ok_or_else(|| "record field range is outside the normalized graph".to_owned())?;
            for field in fields {
                let offset = base_offset
                    .checked_add(field.byte_offset)
                    .ok_or_else(|| "record field offset overflows u16".to_owned())?;
                collect_boundary_scalar_leaves(signature, field.shape, offset, leaves, depth + 1)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn case(variant: &str, payload: Vec<(String, BuildTimeValue)>) -> BuildTimeValue {
    BuildTimeValue::Case {
        variant: variant.to_owned(),
        payload,
    }
}
