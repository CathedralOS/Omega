//! A short-lived storage projection of the canonical type catalog.
//!
//! Foreign boundary references use native pointers/descriptors, not the
//! interpreter's internal reference ABI. Declaration contracts and selection
//! authority remain with the caller; this graph contains storage only.

use arena::Handle;
use language_semantics::boundary_values::{
    BoundaryFieldRecipe, BoundaryRecordStorage, BoundaryValueRecipe, BoundaryValueRecipes,
};
use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage, StructuralTypeShape,
    TerminalModule,
};
use terminal_semantics::StorageGeometryError;

pub struct TerminalBoundaryRecipes<'module> {
    module: &'module TerminalModule,
    target: &'module str,
    extents: &'module dyn Fn(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
    visiting: Vec<StructuralTypeId>,
    closed: Vec<(StructuralTypeId, Handle<BoundaryValueRecipe>)>,
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
            recipes: BoundaryValueRecipes::default(),
        }
    }

    pub fn recipes(&self) -> &BoundaryValueRecipes {
        &self.recipes
    }

    pub fn scalar(&mut self, scalar: ScalarType) -> Result<Handle<BoundaryValueRecipe>, String> {
        let recipe = match scalar {
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
        if let Some((selection_ordinal, selected)) = self
            .module
            .opaque_representations
            .iter()
            .enumerate()
            .find(|(_, row)| row.opaque == structural_type)
        {
            let carrier = self.value(selected.carrier)?;
            return Ok(self.recipes.values.insert(BoundaryValueRecipe::Opaque {
                name: selected.application.opaque_identity.clone(),
                carrier_name: selected.application.carrier_identity.clone(),
                selection_ordinal,
                carrier,
            }));
        }
        if declaration.quotient_carrier.is_some() {
            return Err(
                "nominally opaque boundary value requires retained selected-carrier custody".into(),
            );
        }
        let recipe =
            match &declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => {
                    return self.scalar(*scalar);
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
                StructuralTypeShape::BindingCarrier => {
                    return Err("routed binding has no selected boundary storage".into());
                }
            };
        Ok(self.recipes.values.insert(recipe))
    }

    fn record(
        &mut self,
        record: StructuralTypeId,
        fields: &[StructuralFieldDeclaration],
        storage: StructuralRecordStorage,
    ) -> Result<BoundaryValueRecipe, String> {
        let selected = match storage {
            StructuralRecordStorage::TargetLayout(application) => {
                let application = self
                    .module
                    .target_layout_applications
                    .iter()
                    .find(|candidate| candidate.id == application && candidate.record == record)
                    .ok_or("boundary record has no exact target layout application")?;
                Some(
                    application
                        .realizations
                        .iter()
                        .find(|row| row.target == self.target)
                        .ok_or("boundary record has no layout for its selected target")?,
                )
            }
            _ => None,
        };
        let authored = match storage {
            StructuralRecordStorage::Packed => None,
            StructuralRecordStorage::Authored(storage) => Some(storage),
            StructuralRecordStorage::TargetLayout(_) => selected.map(|row| row.storage),
            StructuralRecordStorage::AccessView(_) | StructuralRecordStorage::Accessor(_) => {
                return Err("placed data has no ordinary owned boundary storage".into());
            }
        };
        let mut recipes = Vec::new();
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let storage = match selected {
                Some(row) => Some(
                    row.fields
                        .iter()
                        .find(|candidate| candidate.field == field.id)
                        .ok_or("target boundary layout omitted a live field")?
                        .storage
                        .clone(),
                ),
                None => field.storage.clone(),
            };
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
            storage: authored.map(|storage| BoundaryRecordStorage {
                byte_size: storage.byte_size,
                byte_alignment: storage.byte_alignment,
            }),
        })
    }
}
