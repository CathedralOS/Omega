/*
We construct bounded native boundary-value signature graphs (`BoundaryValueLayout`)
and short-lived Terminal storage projections (`TerminalBoundaryRecipes`) from
Psi's closed value recipes and canonical Terminal structural types. We pack
fields for a concrete target, classify homogeneous float aggregates, and record
exact per-occurrence opaque selection coordinates without choosing an opaque
representation or authorizing boundary crossings.
*/

use arena::Handle;
use calling_conventions::{
    BoundaryValueClass, BoundaryValueField, BoundaryValueGraph, BoundaryValueShape, ValueClass,
    ValueShape,
};
use language_semantics::boundary_values::{
    BoundaryFieldRecipe, BoundaryRecordStorage, BoundaryValueRecipe, BoundaryValueRecipes,
};
use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralFieldId, StructuralTypeId};
use target::NativeTarget;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage, StructuralTypeShape,
    TerminalModule,
};
use terminal_semantics::StorageGeometryError;

pub const BOUNDARY_VALUE_SHAPE_CAPACITY: usize = 256;
pub const BOUNDARY_VALUE_FIELD_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryOpaqueOccurrence {
    pub selection_ordinal: usize,
    pub shape_root: u16,
}

pub struct BoundaryValueLayout {
    target: NativeTarget,
    layout_profile: Option<target::TargetProfile>,
    shapes: Vec<BoundaryValueShape>,
    fields: Vec<BoundaryValueField>,
    opaque_occurrences: Vec<BoundaryOpaqueOccurrence>,
}

impl BoundaryValueLayout {
    pub fn new(target: impl Into<crate::LayoutTarget>) -> Self {
        let (target, layout_profile) = target.into().into_parts();
        Self {
            target,
            layout_profile,
            shapes: Vec::new(),
            fields: Vec::new(),
            opaque_occurrences: Vec::new(),
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
        Vec<BoundaryOpaqueOccurrence>,
    ) {
        (self.shapes, self.fields, self.opaque_occurrences)
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
                    crate::fat_descriptor_layout(self.target)
                } else {
                    crate::pointer_layout(self.target)
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
                let (shape, shape_root) = self.value_type(recipes, *carrier).map_err(|reason| format!("selected carrier `{carrier_name}` cannot represent boundary-opaque `{name}` by value: {reason}"))?;
                let use_ = BoundaryOpaqueOccurrence {
                    selection_ordinal: *selection_ordinal,
                    shape_root,
                };
                if !self.opaque_occurrences.contains(&use_) {
                    self.opaque_occurrences.push(use_);
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

pub struct TerminalBoundaryRecipes<'module> {
    module: &'module TerminalModule,
    target: &'module str,
    extents: &'module dyn Fn(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    visiting: Vec<StructuralTypeId>,
    closed: Vec<(StructuralTypeId, Handle<BoundaryValueRecipe>)>,
    fields: Vec<(StructuralTypeId, StructuralFieldId, usize)>,
    recipes: BoundaryValueRecipes,
}

impl<'module> TerminalBoundaryRecipes<'module> {
    pub fn new(
        module: &'module TerminalModule,
        target: &'module str,
        extents: &'module dyn Fn(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    ) -> Self {
        Self {
            module,
            target,
            extents,
            visiting: Vec::new(),
            closed: Vec::new(),
            fields: Vec::new(),
            recipes: BoundaryValueRecipes::default(),
        }
    }

    pub fn recipes(&self) -> &BoundaryValueRecipes {
        &self.recipes
    }

    pub fn record_field(
        &mut self,
        record: StructuralTypeId,
        field: StructuralFieldId,
    ) -> Result<(Handle<BoundaryValueRecipe>, usize), String> {
        let recipe = self.value(record)?;
        let ordinal = self
            .fields
            .iter()
            .find(|(owner, candidate, _)| *owner == record && *candidate == field)
            .map(|(_, _, ordinal)| *ordinal)
            .ok_or("boundary record has no exact live field recipe")?;
        Ok((recipe, ordinal))
    }

    pub fn scalar(&mut self, scalar: ScalarType) -> Result<Handle<BoundaryValueRecipe>, String> {
        let recipe = match scalar {
            ScalarType::Callable(_) => {
                return Err("local machine value requires an admitted boundary proxy".into());
            }
            ScalarType::Boolean => BoundaryValueRecipe::Integer { byte_size: 1 },
            ScalarType::Integer(integer) if integer.bits() % 8 == 0 => {
                BoundaryValueRecipe::Integer {
                    byte_size: integer.bits() / 8,
                }
            }
            ScalarType::Integer(_) => return Err("boundary integer has no fixed byte width".into()),
            ScalarType::IeeeFloat(format) => BoundaryValueRecipe::Float {
                byte_size: format.byte_size(),
            },
        };
        Ok(self.recipes.values.insert(recipe))
    }

    pub fn reference(
        &mut self,
        referent: StructuralTypeId,
    ) -> Result<Handle<BoundaryValueRecipe>, String> {
        let descriptor = self
            .module
            .foreign_reference_descriptor(referent)
            .ok_or("referent has no retained foreign reference representation")?;
        Ok(self
            .recipes
            .values
            .insert(BoundaryValueRecipe::Reference { descriptor }))
    }

    pub fn value(
        &mut self,
        structural_type: StructuralTypeId,
    ) -> Result<Handle<BoundaryValueRecipe>, String> {
        if let Some((_, recipe)) = self
            .closed
            .iter()
            .find(|(candidate, _)| *candidate == structural_type)
        {
            return Ok(*recipe);
        }
        if self.visiting.contains(&structural_type) {
            return Err("recursive by-value Terminal data has no finite boundary shape".into());
        }
        self.visiting.push(structural_type);
        let result = self.value_recipe(structural_type);
        self.visiting.pop();
        let recipe = result?;
        self.closed.push((structural_type, recipe));
        Ok(recipe)
    }

    fn value_recipe(
        &mut self,
        structural_type: StructuralTypeId,
    ) -> Result<Handle<BoundaryValueRecipe>, String> {
        let declaration = self
            .module
            .structural_type(structural_type)
            .ok_or("boundary value has no canonical storage declaration")?;
        if let Some((selection_ordinal, selected)) =
            terminal_semantics::foreign_opaque_carrier(self.module, declaration)?
        {
            let carrier = self.value(selected.carrier)?;
            return Ok(self.recipes.values.insert(BoundaryValueRecipe::Opaque {
                name: selected.application.opaque_identity.clone(),
                carrier_name: selected.application.carrier_identity.clone(),
                selection_ordinal,
                carrier,
            }));
        }
        let recipe =
            match &declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => {
                    return self.scalar(*scalar);
                }
                StructuralTypeShape::AtomicCell(cell) => {
                    return self.scalar(cell.scalar_type());
                }
                StructuralTypeShape::Reference { referent, .. } => {
                    return self.reference(*referent);
                }
                StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::ElementView { .. } => {
                    BoundaryValueRecipe::Reference { descriptor: true }
                }
                StructuralTypeShape::FixedArray { element, length } => {
                    BoundaryValueRecipe::FixedArray {
                        element: self.value(*element)?,
                        length: match length {
                            terminal_psi::ArrayExtent::Literal(count) => *count,
                            terminal_psi::ArrayExtent::Application(application) => {
                                (self.extents)(*application).map_err(|error| error.to_string())?
                            }
                        },
                    }
                }
                StructuralTypeShape::Record { fields, storage } => {
                    self.record(structural_type, fields, *storage)?
                }
                StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. } => return Err(
                    "case data is not yet classifiable as a boundary value; pass it by reference"
                        .into(),
                ),
            };
        Ok(self.recipes.values.insert(recipe))
    }

    fn record(
        &mut self,
        record: StructuralTypeId,
        fields: &[StructuralFieldDeclaration],
        storage: StructuralRecordStorage,
    ) -> Result<BoundaryValueRecipe, String> {
        let selected =
            terminal_semantics::foreign_record_storage(self.module, record, storage, self.target)?;
        let mut recipes = Vec::new();
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let storage = selected.field(field)?;
            let value = match storage.as_ref().and_then(|storage| storage.stored_scalar()) {
                Some(scalar) => self.scalar(scalar)?,
                None => match &field.field_type {
                    StructuralFieldType::Structural(child) => self.value(*child)?,
                    kind if kind.scalar_type().is_some() => self.scalar(
                        kind.scalar_type()
                            .ok_or("boundary scalar field lost its carrier")?,
                    )?,
                    kind if kind.byte_sequence_carrier().is_some() => self
                        .recipes
                        .values
                        .insert(BoundaryValueRecipe::Reference { descriptor: true }),
                    _ => return Err("boundary field has no executable storage recipe".into()),
                },
            };
            if storage
                .as_ref()
                .and_then(|storage| storage.element_stride())
                .is_some()
            {
                return Err(
                    "foreign calling recipes do not yet retain authored repeated-field stride"
                        .into(),
                );
            }
            let byte_offset = storage
                .as_ref()
                .map(|storage| {
                    storage
                        .byte_offset()
                        .ok_or("fragmented field has no single foreign boundary offset")
                })
                .transpose()?;
            self.fields.push((record, field.id, recipes.len()));
            recipes.push(BoundaryFieldRecipe { value, byte_offset });
        }
        Ok(BoundaryValueRecipe::Record {
            name: self
                .module
                .identities
                .text(
                    self.module
                        .structural_type(record)
                        .ok_or("boundary record disappeared")?
                        .identity,
                )
                .ok_or("boundary record lost its declaration identity")?
                .to_owned(),
            fields: self.recipes.fields.insert_many(recipes),
            storage: selected.storage.map(|storage| BoundaryRecordStorage {
                byte_size: storage.byte_size,
                byte_alignment: storage.byte_alignment,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BOUNDARY_VALUE_SHAPE_CAPACITY, BoundaryValueLayout, TerminalBoundaryRecipes,
        append_boundary_value_shape,
    };
    use calling_conventions::{BoundaryValueClass, BoundaryValueShape, ValueShape};
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;
    use typed_trees::TypedTrees;

    #[test]
    fn canonical_array_recipes_consume_only_completed_extent_inputs() {
        use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};
        use terminal_psi::{
            ArrayExtent, StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
        };
        let mut module = terminal_psi::TerminalModule::declarations();
        let element = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let application = ScalarApplicationId::new(1).unwrap();
        for (id, shape) in [
            (
                element,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            (
                array,
                StructuralTypeShape::FixedArray {
                    element,
                    length: ArrayExtent::Application(application),
                },
            ),
        ] {
            module.structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id,
                identity: TerminalIdentityIndex::ABSENT,
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                shape,
                scalar_restriction: None,
            });
        }
        for count in [2, 5] {
            let extent = |requested| {
                assert_eq!(requested, application);
                Ok(count)
            };
            let mut recipes = TerminalBoundaryRecipes::new(&module, "macos_arm64", &extent);
            let recipe = recipes.value(array).unwrap();
            let mut layout =
                BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
            let (shape, _) = layout.value_type(recipes.recipes(), recipe).unwrap();
            assert_eq!(u64::from(shape.byte_size), count);
            assert_eq!(shape.alignment, 1);
        }
        let mut pending = TerminalBoundaryRecipes::new(&module, "macos_arm64", &|requested| {
            Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                requested,
            ))
        });
        assert!(
            pending
                .value(array)
                .unwrap_err()
                .contains("UnresolvedExtent")
        );
    }

    #[test]
    fn identity_only_reference_transport_closes_geometry_but_not_owned_storage() {
        for descriptor in [false, true] {
            let referent = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
            let mut module = terminal_psi::TerminalModule::declarations();
            module
                .structural_referents
                .push(terminal_psi::StructuralReferentIdentity {
                    id: referent,
                    identity: terminal_psi::TerminalIdentityIndex::ABSENT,
                    foreign_descriptor: descriptor,
                    abstract_opaque: false,
                });
            let mut recipes =
                TerminalBoundaryRecipes::new(&module, "macos_arm64", &|application| {
                    Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                        application,
                    ))
                });
            let recipe = recipes
                .reference(referent)
                .expect("explicit transport role");
            let mut layout =
                BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
            let (shape, _) = layout
                .value_type(recipes.recipes(), recipe)
                .expect("target geometry");
            assert_eq!(
                shape,
                ValueShape::integer(if descriptor { 16 } else { 8 }, 8)
            );
            let error = recipes
                .value(referent)
                .expect_err("transport supplied owned storage");
            assert!(
                error.contains("no canonical storage declaration"),
                "{error}"
            );
            let unknown = semantic_vocabulary::StructuralTypeId::new(2).unwrap();
            assert!(
                recipes
                    .reference(unknown)
                    .unwrap_err()
                    .contains("no retained foreign reference representation")
            );
        }
    }

    fn typed(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokens");
        let syntax = parse_syntax_trees(&tokens).expect("syntax");
        let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolved");
        lower_symbol_resolved_trees(&resolved).expect("typed")
    }

    fn symbol(typed: &TypedTrees, name: &str) -> symbols::SymbolHandle {
        typed
            .data_definition_by_name_str(name)
            .expect("fixture declaration")
            .symbol
    }

    fn data_recipes(
        typed: &TypedTrees,
        names: &[&str],
    ) -> (
        language_semantics::boundary_values::BoundaryValueRecipes,
        Vec<arena::Handle<language_semantics::boundary_values::BoundaryValueRecipe>>,
    ) {
        let mut preparation =
            typed_trees::boundary_values::BoundaryValuePreparation::new(typed, &[], &[]);
        let roots = names
            .iter()
            .map(|name| preparation.data(symbol(typed, name), name))
            .collect();
        (preparation.finish(), roots)
    }

    #[test]
    fn records_preserve_padding_float_classification_and_distinct_occurrences() {
        let typed =
            typed("data Padded { tag: u8; value: u64; } data Floats { left: f32; right: f32; }");
        let (recipes, roots) = data_recipes(&typed, &["Padded", "Floats", "Padded"]);
        drop(typed);
        let mut values =
            BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
        let (padded, padded_root) = values.value_type(&recipes, roots[0]).expect("padded");
        assert_eq!(padded, ValueShape::integer(16, 8));
        let (floats, floats_root) = values.value_type(&recipes, roots[1]).expect("floats");
        assert_eq!(floats, ValueShape::homogeneous_float_aggregate(4, 2));
        let (_, repeated_root) = values.value_type(&recipes, roots[2]).expect("repeated");
        assert_ne!(padded_root, repeated_root);
        let graph = values.graph();
        for (root, offsets) in [
            (padded_root, [0, 8]),
            (floats_root, [0, 4]),
            (repeated_root, [0, 8]),
        ] {
            let BoundaryValueClass::Record {
                first_field,
                field_count,
            } = graph.shapes[usize::from(root)].class
            else {
                panic!("record root");
            };
            assert_eq!(field_count, 2);
            let fields = &graph.fields[usize::from(first_field)..usize::from(first_field) + 2];
            assert_eq!([fields[0].byte_offset, fields[1].byte_offset], offsets);
        }
    }

    #[test]
    fn arrays_and_unsized_references_retain_their_structural_classes() {
        let typed = typed(
            "domain [u8]::Utf8 requires valid_utf8(self); \
             data Values { words: [u16; 3]; slice: &[u8]; text: &[u8] in Utf8; scalar: &u64; }",
        );
        let (recipes, roots) = data_recipes(&typed, &["Values"]);
        drop(typed);
        let mut values = BoundaryValueLayout::new(target::TargetProfile::UefiX64.native_target());
        let (shape, root) = values.value_type(&recipes, roots[0]).expect("values");
        assert_eq!(shape, ValueShape::integer(48, 8));
        let graph = values.graph();
        let BoundaryValueClass::Record { first_field, .. } = graph.shapes[usize::from(root)].class
        else {
            panic!("record");
        };
        let fields = &graph.fields[usize::from(first_field)..];
        let array = graph.shapes[usize::from(fields[0].shape)];
        let BoundaryValueClass::FixedArray { element, length } = array.class else {
            panic!("array");
        };
        assert_eq!(length, 3);
        assert_eq!(array.byte_size, 6);
        assert_eq!(graph.shapes[usize::from(element)].byte_size, 2);
        for (field, expected_size) in fields[1..].iter().zip([16, 16, 8]) {
            let child = graph.shapes[usize::from(field.shape)];
            assert_eq!(child.class, BoundaryValueClass::Reference);
            assert_eq!(child.byte_size, expected_size);
        }
    }

    #[test]
    fn unsupported_by_value_carriers_fail_at_layout_construction() {
        for (source, name, reason) in [
            (
                "data Recursive { next: Recursive; }",
                "Recursive",
                "recursive by-value",
            ),
            (
                "data TooWide { words: [u64; 8192]; }",
                "TooWide",
                "exceeds 65535 bytes",
            ),
            ("data Empty {}", "Empty", "zero-sized"),
        ] {
            let typed = typed(source);
            let (recipes, roots) = data_recipes(&typed, &[name]);
            drop(typed);
            let mut values =
                BoundaryValueLayout::new(target::TargetProfile::UefiX64.native_target());
            let error = values
                .value_type(&recipes, roots[0])
                .expect_err("unsupported carrier");
            assert!(error.contains(reason), "{error}");
        }
        let typed = typed("boundary data Opaque;");
        let (recipes, roots) = data_recipes(&typed, &["Opaque"]);
        drop(typed);
        let mut values = BoundaryValueLayout::new(target::TargetProfile::UefiX64.native_target());
        assert!(
            values
                .value_type(&recipes, roots[0])
                .expect_err("unselected opaque")
                .contains("selects no exact")
        );
    }

    #[test]
    fn shape_capacity_is_checked_before_publishing_an_out_of_range_root() {
        let shape = BoundaryValueShape {
            class: BoundaryValueClass::Integer,
            byte_size: 8,
            alignment: 8,
        };
        let mut shapes = vec![shape; BOUNDARY_VALUE_SHAPE_CAPACITY - 1];
        assert_eq!(
            append_boundary_value_shape(&mut shapes, shape).expect("last shape"),
            255
        );
        let error = append_boundary_value_shape(&mut shapes, shape).expect_err("capacity");
        assert!(error.contains("normalized shape capacity of 256"));
        assert_eq!(shapes.len(), BOUNDARY_VALUE_SHAPE_CAPACITY);
    }

    #[test]
    fn opaque_recipes_keep_exact_roster_coordinates_through_nested_occurrences() {
        use language_semantics::boundary_values::{
            BoundaryFieldRecipe, BoundaryValueRecipe, BoundaryValueRecipes,
        };
        let mut recipes = BoundaryValueRecipes::default();
        let word = recipes
            .values
            .insert(BoundaryValueRecipe::Integer { byte_size: 8 });
        let opaque = recipes.values.insert(BoundaryValueRecipe::Opaque {
            name: "Token".into(),
            carrier_name: "TokenBytes".into(),
            selection_ordinal: 41,
            carrier: word,
        });
        let array = recipes.values.insert(BoundaryValueRecipe::FixedArray {
            element: opaque,
            length: 3,
        });
        let fields = recipes.fields.insert_many([
            BoundaryFieldRecipe {
                value: opaque,
                byte_offset: None,
            },
            BoundaryFieldRecipe {
                value: array,
                byte_offset: None,
            },
        ]);
        let record = recipes.values.insert(BoundaryValueRecipe::Record {
            name: "Envelope".into(),
            fields,
            storage: None,
        });
        let mut values =
            BoundaryValueLayout::new(target::TargetProfile::MacosArm64.native_target());
        let (shape, _) = values
            .value_type(&recipes, record)
            .expect("nested occurrence geometry");
        assert_eq!(shape, ValueShape::integer(32, 8));
        values
            .value_type(&recipes, opaque)
            .expect("another top-level occurrence");
        let (shapes, _, occurrences) = values.into_parts();
        assert_eq!(occurrences.len(), 3);
        for occurrence in &occurrences {
            assert_eq!(occurrence.selection_ordinal, 41);
            assert_eq!(shapes[usize::from(occurrence.shape_root)].byte_size, 8);
        }
        assert!(
            occurrences
                .windows(2)
                .all(|pair| pair[0].shape_root != pair[1].shape_root)
        );
    }

    #[test]
    fn plan_laid_fields_use_selected_offsets_and_stored_width_with_exact_member_custody() {
        use typed_trees::typed_trees::{PlanLaidIntegerField, PlanLaidLayout};
        let mut typed = typed("data Packed { value: u64; }");
        let definition = typed.data_definitions()[0].clone();
        let typed_trees::data::DataMember::Field(field) = &typed.data_members(&definition)[0]
        else {
            panic!("field");
        };
        let field_symbol = field.symbol;
        let application =
            typed
                .type_reference_table
                .insert(typed_trees::types::TypeReferenceNode::Named {
                    symbol: definition.symbol,
                    name: definition.name.clone(),
                });
        let declarations = typed.roots.data_definitions;
        typed.data_definitions.span_mut_or_empty(declarations)[0].generic_instance =
            Some(application);
        typed.plan_laid_layouts.push(PlanLaidLayout {
            application,
            semantic_type: Default::default(),
            policy_application: Default::default(),
            target_inputs: Vec::new(),
            data_name: "Packed".into(),
            data_symbol: definition.symbol,
            field_symbols: vec![field_symbol],
            schema_symbol: definition.symbol,
            schema_field_symbols: vec![field_symbol],
            policy_symbol: symbols::SymbolHandle::invalid(),
            policy_plan_machine_symbol: symbols::SymbolHandle::invalid(),
            schema_argument: None,
            realizations: typed_trees::PlanLaidRealizations::Invariant(
                typed_trees::PlanLaidGeometry {
                    validated_layout: layout_plans::LayoutPlanReport {
                        schema_report_fingerprint: 1,
                        entries: vec![layout_plans::LayoutFieldEntryReport {
                            field: "value".into(),
                            member_identity: None,
                            placement: layout_plans::LayoutPlacementReport::IntegerAt {
                                offset: 4,
                                stored_width: 16,
                                interpretation: layout_plans::IntegerInterpretation::Unsigned,
                            },
                        }],
                        offsets: Some(vec![4]),
                        size: Some(8),
                        align: 4,
                    },
                    private_callback_demands: vec![],
                    offsets: vec![4],
                    bit_fields: vec![],
                    integer_fields: vec![PlanLaidIntegerField {
                        field_index: 0,
                        stored_width_bits: 16,
                        interpretation: layout_plans::IntegerInterpretation::Unsigned,
                        write_is_total: false,
                    }],
                    repeated_fields: vec![],
                    size: 8,
                    align: 4,
                },
            ),
        });
        let target = target::TargetProfile::UefiX64.native_target();
        let (recipes, roots) = data_recipes(&typed, &["Packed"]);
        let mut values = BoundaryValueLayout::new(target);
        let (shape, _) = values
            .value_type(&recipes, roots[0])
            .expect("selected layout");
        assert_eq!(shape, ValueShape::integer(8, 4));
        assert_eq!(values.graph().fields[0].byte_offset, 4);
        assert_eq!(values.graph().shapes[0].byte_size, 2);
        drop(values);

        let typed_trees::PlanLaidRealizations::Invariant(geometry) =
            &mut typed.plan_laid_layouts[0].realizations
        else {
            panic!("invariant fixture");
        };
        geometry.integer_fields[0].stored_width_bits = 7;
        let (recipes, roots) = data_recipes(&typed, &["Packed"]);
        let error = BoundaryValueLayout::new(target)
            .value_type(&recipes, roots[0])
            .expect_err("invalid width");
        assert!(error.contains("invalid stored-integer width 7"));
        let typed_trees::PlanLaidRealizations::Invariant(geometry) =
            &mut typed.plan_laid_layouts[0].realizations
        else {
            panic!("invariant fixture");
        };
        geometry.integer_fields[0].stored_width_bits = 16;
        typed.plan_laid_layouts[0].field_symbols[0] = symbols::SymbolHandle::invalid();
        let (recipes, roots) = data_recipes(&typed, &["Packed"]);
        let error = BoundaryValueLayout::new(target)
            .value_type(&recipes, roots[0])
            .expect_err("wrong member identity");
        assert!(error.contains("layout publishes"));
    }
}
