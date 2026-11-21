//! Describe the physical leaves a host may transfer at a structural boundary.
//! A descriptor is opaque, an array is described once with a stride, and a sum
//! exposes its tag and the fields of each case. Missing geometry has no recipe.

use crate::PackedFieldLayout;
use crate::terminal_layout::{NativeGeometry, checked_align_up_u32};
use runtime_abi::{NativeBorrowedStorageLayout, NativeStorageCaseTag, NativeStorageRepetition};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, ScalarType, StructuralTypeId};
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};

impl NativeGeometry<'_> {
    pub fn borrowed_storage(&self, root: StructuralTypeId) -> Option<NativeBorrowedStorageLayout> {
        let shape = self.shape(root).ok()?;
        let mut recipes = StorageRecipes {
            geometry: self,
            path: Vec::new(),
            repetitions: Vec::new(),
            active: Vec::new(),
            fields: Vec::new(),
            discriminators: Vec::new(),
            opaque_regions: Vec::new(),
        };
        recipes.collect(root, 0, None)?;
        Some(NativeBorrowedStorageLayout::new(
            root,
            shape.byte_size,
            shape.alignment,
            recipes.fields,
            recipes.discriminators,
            recipes.opaque_regions,
        ))
    }
}

type Field = (
    Vec<Path>,
    Vec<NativeStorageRepetition>,
    u32,
    ScalarType,
    ScalarType,
);
type Discriminator = (
    Vec<Path>,
    Vec<NativeStorageRepetition>,
    StructuralTypeId,
    u32,
    Vec<NativeStorageCaseTag>,
);
type OpaqueRegion = (Vec<Path>, Vec<NativeStorageRepetition>, u32, u32);

struct StorageRecipes<'geometry, 'types> {
    geometry: &'geometry NativeGeometry<'types>,
    path: Vec<Path>,
    repetitions: Vec<NativeStorageRepetition>,
    active: Vec<StructuralTypeId>,
    fields: Vec<Field>,
    discriminators: Vec<Discriminator>,
    opaque_regions: Vec<OpaqueRegion>,
}

impl StorageRecipes<'_, '_> {
    fn collect(
        &mut self,
        root: StructuralTypeId,
        offset: u32,
        occurrence_stride: Option<u32>,
    ) -> Option<()> {
        // Explicit record sizes do not by themselves prove finite nesting.
        if self.active.contains(&root) {
            return None;
        }
        self.active.push(root);
        let declaration = self.geometry.declaration(root).ok()?;
        match &declaration.shape {
            StructuralTypeShape::TypeParameter | StructuralTypeShape::Opaque => return None,
            StructuralTypeShape::PrimitiveScalar(scalar) => {
                self.fields.push((
                    self.path.clone(),
                    self.repetitions.clone(),
                    offset,
                    *scalar,
                    *scalar,
                ));
            }
            StructuralTypeShape::AtomicCell(cell) => {
                let scalar = cell.scalar_type();
                self.fields.push((
                    self.path.clone(),
                    self.repetitions.clone(),
                    offset,
                    scalar,
                    scalar,
                ));
            }
            StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::ElementView { .. } => {
                let shape = self.geometry.shape(root).ok()?;
                self.opaque_regions.push((
                    self.path.clone(),
                    self.repetitions.clone(),
                    offset,
                    shape.byte_size,
                ));
            }
            StructuralTypeShape::Record { fields, .. } => {
                self.members(root, fields, None, offset)?;
            }
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                let common = match &declaration.shape {
                    StructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                    _ => &[],
                };
                let layout = self.geometry.sum_layout(root).ok()?;
                let tags = cases
                    .iter()
                    .enumerate()
                    .map(|(ordinal, case)| {
                        Some(NativeStorageCaseTag {
                            case: case.id,
                            native_tag: i32::try_from(ordinal).ok()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                self.discriminators.push((
                    self.path.clone(),
                    self.repetitions.clone(),
                    root,
                    offset.checked_add(layout.tag_byte_offset)?,
                    tags,
                ));
                self.members(root, common, Some(&layout.common_fields), offset)?;
                for (case, placement) in cases.iter().zip(layout.cases) {
                    self.path.push(Path::Case(case.id));
                    self.members(root, &case.fields, Some(&placement.fields), offset)?;
                    self.path.pop();
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let length = length.literal()?;
                let shape = self.geometry.shape(*element).ok()?;
                let stride = occurrence_stride.or_else(|| {
                    checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))
                })?;
                self.repetitions.push(NativeStorageRepetition {
                    path_position: self.path.len(),
                    length,
                    byte_stride: stride,
                });
                self.path.push(Path::FixedIndex(0));
                self.collect(*element, offset, None)?;
                self.path.pop();
                self.repetitions.pop();
            }
        }
        self.active.pop();
        Some(())
    }

    fn members(
        &mut self,
        root: StructuralTypeId,
        members: &[StructuralFieldDeclaration],
        packed: Option<&[PackedFieldLayout]>,
        offset: u32,
    ) -> Option<()> {
        let mut cursor = 0;
        for (index, field) in members
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .enumerate()
        {
            if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
                continue;
            }
            let (placement, stride) = if let Some(packed) = packed {
                // The sum layout defines all member offsets. There is no
                // second authored placement for a live sum field.
                if field.storage.is_some() {
                    return None;
                }
                (*packed.get(index)?, None)
            } else {
                let placement = self.geometry.field_placement(field, root, cursor).ok()?;
                if field.storage.is_none() {
                    cursor = placement
                        .byte_offset
                        .checked_add(placement.shape.byte_size)?;
                }
                (
                    PackedFieldLayout {
                        shape: placement.shape,
                        byte_offset: placement.byte_offset,
                    },
                    placement.element_stride,
                )
            };
            let start = offset.checked_add(placement.byte_offset)?;
            self.path.push(Path::Field(field.id));
            if let Some(scalar) = field.field_type.scalar_type() {
                let stored = field
                    .storage
                    .as_ref()
                    .and_then(|storage| storage.stored_scalar())
                    .unwrap_or(scalar);
                self.fields.push((
                    self.path.clone(),
                    self.repetitions.clone(),
                    start,
                    scalar,
                    stored,
                ));
            } else if let Some(child) = field.field_type.as_structural() {
                self.collect(child, start, stride)?;
            } else {
                field.field_type.byte_sequence_carrier()?;
                self.opaque_regions.push((
                    self.path.clone(),
                    self.repetitions.clone(),
                    start,
                    placement.shape.byte_size,
                ));
            }
            self.path.pop();
        }
        Some(())
    }
}
