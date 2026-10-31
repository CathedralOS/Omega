//! Native boundary value layout from an already-typed signature application.
//!
//! This owner constructs scalar/reference extents, ordered aggregate field
//! graphs and exact selected opaque carrier occurrences. It does not select a
//! signature, authorize a representation, evaluate a calling policy or assign
//! argument registers; callers supply those admitted inputs separately.

mod opaque_representations;
pub use opaque_representations::BoundaryOpaqueRepresentationUse;
#[cfg(test)]
mod tests;

use crate::TypeLayout;
use crate::sizing::{fat_descriptor_layout, pointer_layout};
use calling_conventions::{
    BoundaryValueClass, BoundaryValueField, BoundaryValueGraph, BoundaryValueShape, ValueClass,
    ValueShape,
};
use representation_selections::{OpaqueRepresentationSelection, selection_for_opaque};
use target::NativeTarget;
use typed_trees::TypedTrees;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Bounds shared with the source calling-policy graph encoding.
pub const BOUNDARY_VALUE_SHAPE_CAPACITY: usize = 256;
pub const BOUNDARY_VALUE_FIELD_CAPACITY: usize = 256;
use self::{
    BOUNDARY_VALUE_FIELD_CAPACITY as VALUE_FIELD_CAPACITY,
    BOUNDARY_VALUE_SHAPE_CAPACITY as VALUE_SHAPE_CAPACITY,
};

/// Type arguments of the boundary application selected by the caller.
/// Names retain the existing unresolved-parameter compatibility lookup.
#[derive(Clone)]
pub struct BoundaryTypeBinding {
    pub parameter_symbol: symbols::SymbolHandle,
    pub parameter_name: String,
    pub actual: TypeReferenceHandle,
}

/// One invocation's construction storage. Roots identify occurrences, not
/// interned equal layouts; field order and opaque selection custody survive
/// through `into_parts`. Discard the builder after a construction error.
pub struct BoundaryValueLayout<'input> {
    typed: &'input TypedTrees,
    target: NativeTarget,
    bindings: &'input [BoundaryTypeBinding],
    selections: &'input [OpaqueRepresentationSelection],
    visiting: Vec<DataApplicationKey>,
    shapes: Vec<BoundaryValueShape>,
    fields: Vec<BoundaryValueField>,
    opaque_representations: Vec<BoundaryOpaqueRepresentationUse>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DataApplicationKey {
    Nominal(symbols::SymbolHandle),
    Applied(typed_trees::type_identity::TypeIdentityId),
}

impl<'input> BoundaryValueLayout<'input> {
    /// Borrow the already-selected signature inputs; no admission or policy
    /// evaluation occurs here. Sizes and offsets in the output are bytes.
    pub fn new(
        typed: &'input TypedTrees,
        target: NativeTarget,
        bindings: &'input [BoundaryTypeBinding],
        selections: &'input [OpaqueRepresentationSelection],
    ) -> Self {
        Self {
            typed,
            target,
            bindings,
            selections,
            visiting: Vec::new(),
            shapes: Vec::new(),
            fields: Vec::new(),
            opaque_representations: Vec::new(),
        }
    }

    /// Borrow the successfully constructed prefix without changing root indices.
    pub fn graph(&self) -> BoundaryValueGraph<'_> {
        BoundaryValueGraph {
            shapes: &self.shapes,
            fields: &self.fields,
        }
    }

    /// Transfer the graph and its exact opaque-use occurrences to the caller.
    pub fn into_parts(
        self,
    ) -> (
        Vec<BoundaryValueShape>,
        Vec<BoundaryValueField>,
        Vec<BoundaryOpaqueRepresentationUse>,
    ) {
        (self.shapes, self.fields, self.opaque_representations)
    }

    /// Append one concrete type's boundary layout and return its ABI shape and
    /// structural root. Unsupported/open values and exhausted bounds reject.
    pub fn value_type(
        &mut self,
        mut type_reference: TypeReferenceHandle,
    ) -> Result<(ValueShape, u16), String> {
        let typed = self.typed;
        let bindings = self.bindings;
        type_reference = substitute_boundary_type_reference(typed, type_reference, bindings);
        if let Some(primitive) = typed.primitive_type_reference(type_reference) {
            let abi = primitive_value_shape(primitive)?;
            let class = if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                BoundaryValueClass::Float
            } else {
                BoundaryValueClass::Integer
            };
            let root = abi_boundary_shape(&mut self.shapes, class, &abi)?;
            return Ok((abi, root));
        }
        match typed.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => self.value_type(*base_type),
            TypeReferenceNode::Reference { referee, .. } => {
                let mut referee = substitute_boundary_type_reference(typed, *referee, bindings);
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    typed.type_reference_table.type_reference(referee)
                {
                    referee = substitute_boundary_type_reference(typed, *base_type, bindings);
                }
                // An unsized referee needs a two-word descriptor: `{ptr, len}` for
                // slices and `string`, `{instance, table}` for `dyn Trait` — the
                // same pair ordinary dynamic-descriptor arguments expand to.
                let descriptor = match typed.type_reference_table.type_reference(referee) {
                    TypeReferenceNode::Slice { .. } | TypeReferenceNode::DynamicTrait { .. } => {
                        fat_descriptor_layout(self.target)
                    }
                    TypeReferenceNode::Named { name, .. } if name.as_str() == "string" => {
                        fat_descriptor_layout(self.target)
                    }
                    _ => pointer_layout(self.target),
                };
                self.pointer_boundary_shape(descriptor)
            }
            TypeReferenceNode::Slice { .. } => {
                self.pointer_boundary_shape(fat_descriptor_layout(self.target))
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => {
                let (element, element_root) = self.value_type(*element_type)?;
                let size = (element.byte_size as usize)
                    .checked_mul(*length)
                    .ok_or_else(|| "fixed-array boundary shape overflows".to_owned())?;
                let abi = ValueShape::integer(
                    u32::try_from(size)
                        .map_err(|_| "fixed-array boundary shape exceeds 65535 bytes".to_owned())?,
                    element.alignment,
                );
                let root = abi_boundary_shape(
                    &mut self.shapes,
                    BoundaryValueClass::FixedArray {
                        element: element_root,
                        length: u16::try_from(*length).map_err(|_| {
                            "fixed-array boundary length exceeds 65535 elements".to_owned()
                        })?,
                    },
                    &abi,
                )?;
                Ok((abi, root))
            }
            TypeReferenceNode::Named { symbol, name } => self.data(*symbol, name.as_str()),
            TypeReferenceNode::DynamicTrait { .. } => {
                self.pointer_boundary_shape(fat_descriptor_layout(self.target))
            }
            TypeReferenceNode::Generic { .. } => {
                let definition = typed.closed_data_definition(type_reference).ok_or_else(|| format!(
                    "generic value type `{}` is not concrete at the Calling<C> relationship",
                    typed.display_type_reference(type_reference)
                ))?;
                self.plain_data_definition(
                    definition,
                    DataApplicationKey::Applied(typed.normalized_type_identity(type_reference)),
                )
            }
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

    /// Append the ABI shape of one pointer-shaped boundary occurrence from the
    /// extent `sizing` publishes. The word count is never re-derived here; this
    /// owns only the u16 graph coordinates of that extent.
    fn pointer_boundary_shape(&mut self, layout: TypeLayout) -> Result<(ValueShape, u16), String> {
        let byte_size = u16::try_from(layout.size)
            .map_err(|_| "target pointer-shaped extent exceeds boundary shape range".to_owned())?;
        let alignment = u16::try_from(layout.alignment)
            .map_err(|_| "target pointer-shaped extent exceeds boundary shape range".to_owned())?;
        let abi = ValueShape::integer(u32::from(byte_size), alignment);
        let root = abi_boundary_shape(&mut self.shapes, BoundaryValueClass::Reference, &abi)?;
        Ok((abi, root))
    }

    /// Append the semantic carrier of an attached requirement's explicit self.
    pub fn data(
        &mut self,
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> Result<(ValueShape, u16), String> {
        if self
            .typed
            .data_definition_by_symbol(symbol)
            .is_some_and(|definition| {
                definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            })
        {
            self.opaque_data(symbol, name)
        } else {
            self.plain_data(symbol, name)
        }
    }

    fn opaque_data(
        &mut self,
        opaque: symbols::SymbolHandle,
        opaque_name: &str,
    ) -> Result<(ValueShape, u16), String> {
        let typed = self.typed;
        let selections = self.selections;
        if self.visiting.contains(&DataApplicationKey::Nominal(opaque)) {
            return Err(format!(
                "opaque representation for `{opaque_name}` recursively contains its own semantic declaration"
            ));
        }
        let selection = selection_for_opaque(selections, opaque).ok_or_else(|| {
        format!(
            "boundary-opaque data `{opaque_name}` crosses this boundary by value, but the authoritative build selects no exact `OpaqueRepresentation<{opaque_name}, Representation>` implementation"
        )
    })?;
        let application = selection.application();
        let carrier = typed.closed_data_definition(selection.carrier_type())
        .ok_or_else(|| {
            format!(
                "selected opaque representation carrier for `{opaque_name}` disappeared before shape closure"
            )
        })?;
        self.visiting.push(DataApplicationKey::Nominal(opaque));
        let shape = self.value_type(selection.carrier_type());
        self.visiting.pop();
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
        if !self.opaque_representations.contains(&use_identity) {
            self.opaque_representations.push(use_identity);
        }
        Ok((abi, shape_root))
    }

    fn plain_data(
        &mut self,
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> Result<(ValueShape, u16), String> {
        let typed = self.typed;
        let definition = typed
            .data_definition_by_symbol(symbol)
            .ok_or_else(|| format!("named boundary type `{name}` has no data definition"))?;
        self.plain_data_definition(definition, DataApplicationKey::Nominal(symbol))
    }

    fn plain_data_definition(
        &mut self,
        definition: &'input typed_trees::data::DataDefinition,
        key: DataApplicationKey,
    ) -> Result<(ValueShape, u16), String> {
        let typed = self.typed;
        let name = definition.name.as_str();
        if self.visiting.contains(&key) {
            return Err(format!(
                "recursive by-value data `{name}` has no finite boundary shape"
            ));
        }
        let application = typed.plan_laid_layout(definition.symbol);
        let planned_layout = application.map(|layout| {
            layout.invariant_geometry().ok_or_else(|| format!(
                "plan-laid data `{name}` retains an unresolved target-family layout; native boundary classification requires an invariant layout or a target-closed Terminal view"
            ))
        }).transpose()?;
        let runtime_field_symbols = typed
            .live_data_fields(typed.data_members(definition))
            .map(|field| field.symbol)
            .collect::<Vec<_>>();
        if let Some((application, layout)) = application.zip(planned_layout)
            && (layout.offsets.len() != runtime_field_symbols.len()
                || application.field_symbols != runtime_field_symbols)
        {
            return Err(format!(
                "plan-laid data `{name}` has {} members but its layout publishes {} offsets",
                runtime_field_symbols.len(),
                layout.offsets.len()
            ));
        }
        self.visiting.push(key);
        let mut size = 0usize;
        let mut alignment = 1usize;
        let mut float_member_size = None;
        let mut float_members = 0usize;
        let mut record_fields = Vec::new();
        for member in typed.data_members(definition) {
            let Some(field) = member.as_field() else {
                self.visiting.pop();
                return Err(format!(
                    "case data `{name}` is not yet classifiable as a boundary value; pass it by reference"
                ));
            };
            if field.relevance.is_erased() {
                continue;
            }
            let field_index = record_fields.len();
            let (shape, field_root) = if let Some(stored_integer) =
                planned_layout.and_then(|layout| {
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
                    self.visiting.pop();
                    return Err(format!(
                        "plan-laid data `{name}` field `{}` has invalid stored-integer width {}",
                        field.name, stored_integer.stored_width_bits
                    ));
                }
                let shape = ValueShape::integer(u32::from(stored_byte_size), stored_byte_size);
                let root = append_boundary_value_shape(
                    &mut self.shapes,
                    BoundaryValueShape {
                        class: BoundaryValueClass::Integer,
                        byte_size: u16::try_from(shape.byte_size)
                            .map_err(|_| "boundary shape exceeds 65535 bytes".to_owned())?,
                        alignment: shape.alignment,
                    },
                )?;
                (shape, root)
            } else {
                self.value_type(field.type_reference)?
            };
            let field_alignment = usize::from(shape.alignment);
            let field_offset = planned_layout.map_or_else(
                || align_up(size, field_alignment),
                |layout| layout.offsets[field_index],
            );
            size = size.max(
                field_offset
                    .checked_add(shape.byte_size as usize)
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
        self.visiting.pop();
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
                usize::from(size) == (member_size as usize) * float_members
                    && u32::from(alignment) == member_size
                    && record_fields.iter().enumerate().all(|(index, field)| {
                        usize::from(field.byte_offset) == index * (member_size as usize)
                    })
            }) {
            ValueShape::homogeneous_float_aggregate(
                u16::try_from(float_member_size.expect("float member count has size"))
                    .map_err(|_| "float member size exceeds boundary shape range".to_owned())?,
                u8::try_from(float_members).expect("HFA member count fits u8"),
            )
        } else {
            ValueShape::integer(u32::from(size), alignment)
        };
        if self.fields.len().saturating_add(record_fields.len()) > VALUE_FIELD_CAPACITY {
            return Err(format!(
                "boundary signature exceeds the normalized field capacity of {VALUE_FIELD_CAPACITY}"
            ));
        }
        let first_field =
            u16::try_from(self.fields.len()).expect("boundary field capacity fits u16");
        let field_count = u16::try_from(record_fields.len())
            .map_err(|_| format!("data `{name}` has more than 65535 boundary fields"))?;
        self.fields.extend(record_fields);
        let root = append_boundary_value_shape(
            &mut self.shapes,
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
}

/// Substitute the caller's boundary type arguments without selecting a new
/// signature or creating type nodes.
pub fn substitute_boundary_type_reference(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    bindings: &[BoundaryTypeBinding],
) -> TypeReferenceHandle {
    let TypeReferenceNode::Named { symbol, name } =
        typed.type_reference_table.type_reference(type_reference)
    else {
        return type_reference;
    };
    bindings
        .iter()
        .find(|binding| {
            (binding.parameter_symbol.is_valid() && binding.parameter_symbol == *symbol)
                || binding.parameter_name == name.as_str()
        })
        .map_or(type_reference, |binding| binding.actual)
}

/// Append one normalized shape while enforcing the calling-policy graph bound.
pub fn append_boundary_value_shape(
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

fn abi_boundary_shape(
    shapes: &mut Vec<BoundaryValueShape>,
    class: BoundaryValueClass,
    abi: &ValueShape,
) -> Result<u16, String> {
    append_boundary_value_shape(
        shapes,
        BoundaryValueShape {
            class,
            byte_size: u16::try_from(abi.byte_size)
                .map_err(|_| "boundary shape exceeds 65535 bytes".to_owned())?,
            alignment: abi.alignment,
        },
    )
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
        _ => ValueShape::integer(u32::from(byte_size), byte_size.clamp(1, 8)),
    })
}

fn align_up(value: usize, alignment: usize) -> usize {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|value| value / alignment * alignment)
        .unwrap_or(usize::MAX)
}
