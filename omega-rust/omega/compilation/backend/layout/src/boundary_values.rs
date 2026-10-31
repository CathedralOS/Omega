//! Native boundary storage from Psi's closed value recipes.
//!
//! Source formation and substitution belong to Psi. This owner packs fields
//! for a target and preserves exact occurrence coordinates. It neither chooses
//! an opaque representation nor authorizes the resulting boundary crossing.

mod opaque_representations;
pub use opaque_representations::BoundaryOpaqueRepresentationUse;
#[cfg(test)]
mod tests;

use arena::Handle;
use calling_conventions::{
    BoundaryValueClass, BoundaryValueField, BoundaryValueGraph, BoundaryValueShape, ValueClass,
    ValueShape,
};
use language_semantics::boundary_values::{
    BoundaryFieldRecipe, BoundaryRecordStorage, BoundaryValueRecipe, BoundaryValueRecipes,
};
use target::NativeTarget;

pub const BOUNDARY_VALUE_SHAPE_CAPACITY: usize = 256;
pub const BOUNDARY_VALUE_FIELD_CAPACITY: usize = 256;

/// Invocation-owned native graph. Recipe handles belong to the borrowed graph;
/// equal recipes still produce distinct native roots.
pub struct BoundaryValueLayout<'input> {
    target: NativeTarget,
    layout_profile: Option<target::TargetProfile>,
    representations: &'input [BoundaryOpaqueRepresentationUse],
    shapes: Vec<BoundaryValueShape>,
    fields: Vec<BoundaryValueField>,
    opaque_representations: Vec<BoundaryOpaqueRepresentationUse>,
}

impl<'input> BoundaryValueLayout<'input> {
    pub fn new(
        target: impl Into<crate::LayoutTarget>,
        representations: &'input [BoundaryOpaqueRepresentationUse],
    ) -> Self {
        let (target, layout_profile) = target.into().into_parts();
        Self {
            target,
            layout_profile,
            representations,
            shapes: Vec::new(),
            fields: Vec::new(),
            opaque_representations: Vec::new(),
        }
    }

    pub fn graph(&self) -> BoundaryValueGraph<'_> {
        BoundaryValueGraph {
            shapes: &self.shapes,
            fields: &self.fields,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<BoundaryValueShape>,
        Vec<BoundaryValueField>,
        Vec<BoundaryOpaqueRepresentationUse>,
    ) {
        (self.shapes, self.fields, self.opaque_representations)
    }

    pub fn value_type(
        &mut self,
        recipes: &BoundaryValueRecipes,
        recipe: Handle<BoundaryValueRecipe>,
    ) -> Result<(ValueShape, u16), String> {
        let recipe = recipes.values.get(recipe);
        match recipe {
            BoundaryValueRecipe::Absent => Err("boundary value recipe is absent".into()),
            BoundaryValueRecipe::Rejected(reason) => Err(reason.clone()),
            BoundaryValueRecipe::Integer { byte_size } => {
                let shape = ValueShape::integer(u32::from(*byte_size), (*byte_size).clamp(1, 8));
                self.append_abi(BoundaryValueClass::Integer, shape)
            }
            BoundaryValueRecipe::Float { byte_size } => {
                self.append_abi(BoundaryValueClass::Float, ValueShape::float(*byte_size))
            }
            BoundaryValueRecipe::Reference { descriptor } => {
                let layout = if *descriptor {
                    crate::sizing::fat_descriptor_layout(self.target)
                } else {
                    crate::sizing::pointer_layout(self.target)
                };
                let byte_size = u16::try_from(layout.size).map_err(|_| {
                    "target pointer-shaped extent exceeds boundary shape range".to_owned()
                })?;
                let alignment = u16::try_from(layout.alignment).map_err(|_| {
                    "target pointer-shaped extent exceeds boundary shape range".to_owned()
                })?;
                self.append_abi(
                    BoundaryValueClass::Reference,
                    ValueShape::integer(u32::from(byte_size), alignment),
                )
            }
            BoundaryValueRecipe::FixedArray { element, length } => {
                let (element, element_root) = self.value_type(recipes, *element)?;
                let size = u64::from(element.byte_size)
                    .checked_mul(*length)
                    .ok_or_else(|| "fixed-array boundary shape overflows".to_owned())?;
                let shape = ValueShape::integer(
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
                    &shape,
                )?;
                Ok((shape, root))
            }
            BoundaryValueRecipe::Record {
                name,
                fields,
                storage,
            } => {
                let fields = recipes.fields.span_or_empty(*fields);
                self.record(recipes, name, fields, storage.as_ref())
            }
            BoundaryValueRecipe::TargetLayouts { name, alternatives } => {
                let target = self.layout_profile.map(target::TargetProfile::target_name);
                let selected = target.and_then(|target| {
                    recipes
                        .targets
                        .span_or_empty(*alternatives)
                        .iter()
                        .find(|row| row.target == target)
                });
                let selected = selected.ok_or_else(|| format!("fixed representation data `{name}` has no admitted layout for boundary target `{}`", target.unwrap_or("<unselected>")))?;
                self.value_type(recipes, selected.value)
            }
            BoundaryValueRecipe::Opaque {
                name,
                carrier_name,
                selection_ordinal,
                carrier,
            } => {
                let selected = self
                    .representations
                    .get(*selection_ordinal)
                    .copied()
                    .ok_or("boundary opaque recipe has no corresponding selected representation")?;
                let (shape, shape_root) = self.value_type(recipes, *carrier).map_err(|reason| format!("selected carrier `{carrier_name}` cannot represent boundary-opaque `{name}` by value: {reason}"))?;
                let use_ = BoundaryOpaqueRepresentationUse {
                    shape_root,
                    ..selected
                };
                if !self.opaque_representations.contains(&use_) {
                    self.opaque_representations.push(use_);
                }
                Ok((shape, shape_root))
            }
        }
    }

    fn append_abi(
        &mut self,
        class: BoundaryValueClass,
        shape: ValueShape,
    ) -> Result<(ValueShape, u16), String> {
        let root = abi_boundary_shape(&mut self.shapes, class, &shape)?;
        Ok((shape, root))
    }

    fn record(
        &mut self,
        recipes: &BoundaryValueRecipes,
        name: &str,
        fields: &[BoundaryFieldRecipe],
        storage: Option<&BoundaryRecordStorage>,
    ) -> Result<(ValueShape, u16), String> {
        let mut size = 0u64;
        let mut alignment = 1u64;
        let mut float_member_size = None;
        let mut float_members = 0usize;
        let mut record_fields = Vec::new();
        for field in fields {
            let (shape, field_root) = self.value_type(recipes, field.value)?;
            let field_alignment = u64::from(shape.alignment);
            let field_offset = field
                .byte_offset
                .unwrap_or_else(|| align_up(size, field_alignment));
            size = size.max(
                field_offset
                    .checked_add(u64::from(shape.byte_size))
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
        if let Some(storage) = storage {
            size = storage.byte_size;
            alignment = storage.byte_alignment;
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
                u64::from(size) == u64::from(member_size) * float_members as u64
                    && u32::from(alignment) == member_size
                    && record_fields.iter().enumerate().all(|(ordinal, field)| {
                        u64::from(field.byte_offset) == ordinal as u64 * u64::from(member_size)
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
        if self.fields.len().saturating_add(record_fields.len()) > BOUNDARY_VALUE_FIELD_CAPACITY {
            return Err(format!(
                "boundary signature exceeds the normalized field capacity of {BOUNDARY_VALUE_FIELD_CAPACITY}"
            ));
        }
        let first_field =
            u16::try_from(self.fields.len()).expect("boundary field capacity fits u16");
        let field_count = u16::try_from(record_fields.len())
            .map_err(|_| format!("data `{name}` has more than 65535 boundary fields"))?;
        self.fields.extend(record_fields);
        let root = abi_boundary_shape(
            &mut self.shapes,
            BoundaryValueClass::Record {
                first_field,
                field_count,
            },
            &abi,
        )?;
        Ok((abi, root))
    }
}

pub fn append_boundary_value_shape(
    shapes: &mut Vec<BoundaryValueShape>,
    shape: BoundaryValueShape,
) -> Result<u16, String> {
    if shapes.len() >= BOUNDARY_VALUE_SHAPE_CAPACITY {
        return Err(format!(
            "boundary signature exceeds the normalized shape capacity of {BOUNDARY_VALUE_SHAPE_CAPACITY}"
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

fn align_up(value: u64, alignment: u64) -> u64 {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|value| value / alignment * alignment)
        .unwrap_or(u64::MAX)
}
